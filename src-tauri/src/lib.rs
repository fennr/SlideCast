pub mod core;
pub mod domain;
mod tauri_commands;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .invoke_handler(tauri::generate_handler![
            tauri_commands::get_pdf_page_count,
            tauri_commands::compose_video,
            tauri_commands::get_ffmpeg_path_configured,
            tauri_commands::set_ffmpeg_path_configured,
            tauri_commands::save_slide_image,
            tauri_commands::build_slides_video,
            tauri_commands::create_temp_dir,
            tauri_commands::build_slides_video_with_durations,
            tauri_commands::probe_video_duration,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
