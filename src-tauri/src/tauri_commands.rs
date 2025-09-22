use crate::core::{apply_quality, build_ffmpeg_args, build_images_to_video_args, validate_request};
use crate::domain::CompositionRequest;
use base64::Engine as _;
use serde::Deserialize;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

#[derive(serde::Serialize, serde::Deserialize, Default)]
struct FfmpegConfig {
    path: Option<String>,
}

fn config_file_path() -> Option<PathBuf> {
    dirs::config_dir().map(|d| d.join("SlideCast").join("config.json"))
}

fn load_config() -> FfmpegConfig {
    if let Some(p) = config_file_path() {
        if let Ok(bytes) = fs::read(&p) {
            if let Ok(cfg) = serde_json::from_slice::<FfmpegConfig>(&bytes) {
                return cfg;
            }
        }
    }
    FfmpegConfig::default()
}

fn save_config(cfg: &FfmpegConfig) -> Result<(), String> {
    if let Some(p) = config_file_path() {
        if let Some(parent) = p.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let bytes = serde_json::to_vec_pretty(cfg).map_err(|e| e.to_string())?;
        fs::write(&p, bytes).map_err(|e| e.to_string())?
    }
    Ok(())
}

fn ffmpeg_path() -> String {
    if let Ok(p) = std::env::var("SLIDECAST_FFMPEG") {
        if !p.is_empty() {
            return p;
        }
    }
    let cfg = load_config();
    if let Some(p) = cfg.path {
        return p;
    }
    if cfg!(target_os = "windows") {
        "ffmpeg.exe".into()
    } else {
        "ffmpeg".into()
    }
}
#[derive(Debug, Deserialize)]
pub struct PdfCountArgs {
    #[serde(alias = "pdfPath")]
    pub pdf_path: String,
}

#[tauri::command]
pub fn get_pdf_page_count(args: PdfCountArgs) -> Result<u32, String> {
    match lopdf::Document::load(&args.pdf_path) {
        Ok(doc) => Ok(doc.get_pages().len() as u32),
        Err(e) => Err(format!("failed to read pdf: {e}")),
    }
}

#[derive(Debug, Deserialize)]
pub struct ComposeArgs {
    pub request: CompositionRequest,
}

#[tauri::command]
pub async fn compose_video(args: ComposeArgs) -> Result<(), String> {
    let req = args.request;
    validate_request(&req).map_err(|e| e.to_string())?;

    let overlay_rel_w = req.overlay_relative_width;
    let position = req.overlay_position;
    let foreground = req.foreground_kind;

    let mut args = build_ffmpeg_args(
        &req.video_path,
        &req.pdf_path,
        &req.output_path,
        overlay_rel_w,
        position,
        foreground,
    );
    apply_quality(&mut args, req.quality);

    // stream progress from ffmpeg through stderr lines with -progress pipe:2
    let mut cmd = Command::new(ffmpeg_path());
    cmd.args(["-progress", "pipe:2"]).args(&args.0);
    let status = cmd.status().map_err(|e| e.to_string())?;
    if !status.success() {
        return Err(format!("ffmpeg failed with status: {status}"));
    }
    Ok(())
}

#[derive(Debug, Deserialize)]
pub struct SaveSlideImageArgs {
    pub output_dir: String,
    pub index: u32,
    pub png_base64: String,
}

#[tauri::command]
pub async fn save_slide_image(args: SaveSlideImageArgs) -> Result<String, String> {
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(&args.png_base64)
        .map_err(|e| e.to_string())?;
    let path = std::path::Path::new(&args.output_dir).join(format!("{:05}.png", args.index));
    std::fs::create_dir_all(&args.output_dir).map_err(|e| e.to_string())?;
    std::fs::write(&path, bytes).map_err(|e| e.to_string())?;
    Ok(path.to_string_lossy().to_string())
}

#[derive(Debug, Deserialize)]
pub struct BuildSlidesVideoArgs {
    pub frames_dir: String,
    pub fps: u32,
    pub output_path: String,
}

#[tauri::command]
pub async fn build_slides_video(args: BuildSlidesVideoArgs) -> Result<(), String> {
    let glob = format!("{}/**/*.png", args.frames_dir);
    let ff = build_images_to_video_args(&glob, args.fps, &args.output_path);
    let status = Command::new(ffmpeg_path())
        .args(&ff.0)
        .status()
        .map_err(|e| e.to_string())?;
    if !status.success() {
        return Err(format!("ffmpeg failed with status: {status}"));
    }
    Ok(())
}

#[tauri::command]
pub fn create_temp_dir(prefix: String) -> Result<String, String> {
    let mut dir = std::env::temp_dir();
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_nanos();
    dir.push(format!("{prefix}-{nanos}"));
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir.to_string_lossy().to_string())
}

#[derive(Debug, Deserialize)]
pub struct BuildSlidesVideoDurationsArgs {
    pub frames_dir: String,
    pub durations: Vec<f64>,
    pub output_path: String,
}

#[tauri::command]
pub async fn build_slides_video_with_durations(
    args: BuildSlidesVideoDurationsArgs,
) -> Result<(), String> {
    let tmp = std::path::Path::new(&args.output_path)
        .parent()
        .map(|p| p.join("segments"))
        .ok_or_else(|| "invalid output path".to_string())?;
    std::fs::create_dir_all(&tmp).map_err(|e| e.to_string())?;

    let mut list = String::new();
    for (i, dur) in args.durations.iter().enumerate() {
        let slide_path =
            std::path::Path::new(&args.frames_dir).join(format!("{:05}.png", i as u32));
        if !slide_path.exists() {
            return Err(format!(
                "missing slide image: {}",
                slide_path.to_string_lossy()
            ));
        }
        let seg_path = tmp.join(format!("seg_{i:05}.mp4"));
        let ff = vec![
            "-y".into(),
            "-loop".into(),
            "1".into(),
            "-t".into(),
            format!("{}", dur),
            "-i".into(),
            slide_path.to_string_lossy().to_string(),
            "-s".into(),
            "1920x1080".into(),
            "-r".into(),
            "30".into(),
            "-c:v".into(),
            "libx264".into(),
            "-pix_fmt".into(),
            "yuv420p".into(),
            seg_path.to_string_lossy().to_string(),
        ];
        let status = Command::new(ffmpeg_path())
            .args(&ff)
            .status()
            .map_err(|e| e.to_string())?;
        if !status.success() {
            return Err(format!("ffmpeg failed creating segment {i}"));
        }
        list.push_str(&format!("file '{}'\n", seg_path.to_string_lossy()));
    }

    let list_path = tmp.join("concat.txt");
    std::fs::write(&list_path, list).map_err(|e| e.to_string())?;
    let ff = vec![
        "-y".into(),
        "-f".into(),
        "concat".into(),
        "-safe".into(),
        "0".into(),
        "-i".into(),
        list_path.to_string_lossy().to_string(),
        "-c".into(),
        "copy".into(),
        args.output_path.clone(),
    ];
    let status = Command::new(ffmpeg_path())
        .args(&ff)
        .status()
        .map_err(|e| e.to_string())?;
    if !status.success() {
        return Err("ffmpeg concat failed".into());
    }
    Ok(())
}

#[tauri::command]
pub fn get_ffmpeg_path_configured() -> Result<Option<String>, String> {
    Ok(load_config().path)
}

#[tauri::command]
pub fn set_ffmpeg_path_configured(path: Option<String>) -> Result<(), String> {
    let mut cfg = load_config();
    cfg.path = path;
    save_config(&cfg)
}

#[derive(Debug, Deserialize)]
pub struct ProbeVideoArgs {
    #[serde(alias = "videoPath")]
    pub video_path: String,
}

#[tauri::command]
pub fn probe_video_duration(args: ProbeVideoArgs) -> Result<f64, String> {
    let output = Command::new(ffmpeg_path())
        .args(["-i", &args.video_path])
        .output()
        .map_err(|e| e.to_string())?;
    let stderr = String::from_utf8_lossy(&output.stderr);
    if let Some(line) = stderr.lines().find(|l| l.contains("Duration:")) {
        if let Some(start) = line.find("Duration:") {
            let part = &line[start + 9..];
            let time_str = part.trim().split(',').next().unwrap_or("").trim();
            let comps: Vec<&str> = time_str.split(':').collect();
            if comps.len() == 3 {
                let h: f64 = comps[0].trim().parse().unwrap_or(0.0);
                let m: f64 = comps[1].trim().parse().unwrap_or(0.0);
                let s: f64 = comps[2].trim().parse().unwrap_or(0.0);
                return Ok(h * 3600.0 + m * 60.0 + s);
            }
        }
    }
    Err("failed to detect duration; ensure ffmpeg is accessible".into())
}
