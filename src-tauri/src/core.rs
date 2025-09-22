use crate::domain::{
    CompositionRequest, ForegroundKind, OverlayPosition, QualityProfile, SlideTiming,
    ValidationError,
};

pub fn validate_timings(timings: &[SlideTiming]) -> Result<(), ValidationError> {
    if timings.is_empty() {
        return Err(ValidationError::EmptyTimings);
    }
    for (expected_index, t) in timings.iter().enumerate() {
        if t.slide_index != expected_index as u32 {
            return Err(ValidationError::InvalidSlideIndices);
        }
    }
    for w in timings.windows(2) {
        if !(w[0].time_seconds < w[1].time_seconds) {
            return Err(ValidationError::NonIncreasingTimings);
        }
    }
    Ok(())
}

pub fn validate_request(req: &CompositionRequest) -> Result<(), ValidationError> {
    if !(req.overlay_relative_width >= 0.05 && req.overlay_relative_width <= 0.5) {
        return Err(ValidationError::OverlayWidthOutOfRange(
            req.overlay_relative_width,
        ));
    }
    validate_timings(&req.timings)
}

#[derive(Debug, Clone, PartialEq)]
pub struct FfmpegArgs(pub Vec<String>);

/// Build ffmpeg arguments for composing picture-in-picture using filter_complex.
/// Assumes that slide video is generated elsewhere and provided as second input.
pub fn build_ffmpeg_args(
    main_video_path: &str,
    overlay_video_path: &str,
    output_path: &str,
    overlay_rel_w: f64,
    position: OverlayPosition,
    foreground: ForegroundKind,
) -> FfmpegArgs {
    let (bg, fg) = match foreground {
        ForegroundKind::Video => ("1:v", "0:v"),
        ForegroundKind::Slides => ("0:v", "1:v"),
    };

    let (ox, oy) = match position {
        OverlayPosition::TopLeft => ("16", "16"),
        OverlayPosition::TopRight => ("W-w-16", "16"),
        OverlayPosition::BottomLeft => ("16", "H-h-16"),
        OverlayPosition::BottomRight => ("W-w-16", "H-h-16"),
    };

    let filter = format!(
        "[{fg}]scale=1920*{overlay_rel_w}:-1[ov];[{bg}]scale=1920:1080:flags=bicubic[bg];[bg][ov]overlay={ox}:{oy}:eval=init,fps=30",
    );

    let args = vec![
        "-y".into(),
        "-hide_banner".into(),
        "-loglevel".into(),
        "warning".into(),
        "-i".into(),
        main_video_path.into(),
        "-i".into(),
        overlay_video_path.into(),
        "-filter_complex".into(),
        filter,
        "-map".into(),
        "0:a?".into(),
        "-c:v".into(),
        "libx264".into(),
        "-pix_fmt".into(),
        "yuv420p".into(),
        "-r".into(),
        "30".into(),
        "-s".into(),
        "1920x1080".into(),
        "-c:a".into(),
        "aac".into(),
        "-b:a".into(),
        "192k".into(),
        "-shortest".into(),
        "-movflags".into(),
        "+faststart".into(),
        output_path.into(),
    ];

    FfmpegArgs(args)
}

pub fn apply_quality(args: &mut FfmpegArgs, quality: QualityProfile) {
    // tweak crf/preset for speed/quality
    let (crf, preset) = match quality {
        QualityProfile::Draft => (32, "veryfast"),
        QualityProfile::Standard => (26, "medium"),
        QualityProfile::High => (20, "slow"),
    };
    // insert before output path
    let v = &mut args.0;
    // Find position of output path (last element)
    let out = v.pop().unwrap_or_default();
    v.extend([
        "-crf".into(),
        crf.to_string(),
        "-preset".into(),
        preset.into(),
    ]);
    v.push(out);
}

pub fn build_images_to_video_args(images_glob: &str, fps: u32, output_path: &str) -> FfmpegArgs {
    let args = vec![
        "-y".into(),
        "-framerate".into(),
        fps.to_string(),
        "-pattern_type".into(),
        "glob".into(),
        "-i".into(),
        images_glob.into(),
        "-s".into(),
        "1920x1080".into(),
        "-c:v".into(),
        "libx264".into(),
        "-pix_fmt".into(),
        "yuv420p".into(),
        output_path.into(),
    ];
    FfmpegArgs(args)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::SlideTiming;

    #[test]
    fn timings_validation_ok() {
        let timings = vec![
            SlideTiming {
                slide_index: 0,
                time_seconds: 0.5,
            },
            SlideTiming {
                slide_index: 1,
                time_seconds: 10.0,
            },
            SlideTiming {
                slide_index: 2,
                time_seconds: 20.0,
            },
        ];
        assert!(validate_timings(&timings).is_ok());
    }

    #[test]
    fn timings_empty_err() {
        let timings = vec![];
        assert!(matches!(
            validate_timings(&timings),
            Err(ValidationError::EmptyTimings)
        ));
    }

    #[test]
    fn timings_non_increasing_err() {
        let timings = vec![
            SlideTiming {
                slide_index: 0,
                time_seconds: 0.5,
            },
            SlideTiming {
                slide_index: 1,
                time_seconds: 1.0,
            },
            SlideTiming {
                slide_index: 2,
                time_seconds: 1.0,
            },
        ];
        assert!(matches!(
            validate_timings(&timings),
            Err(ValidationError::NonIncreasingTimings)
        ));
    }

    #[test]
    fn timings_invalid_indices_err() {
        let timings = vec![
            SlideTiming {
                slide_index: 0,
                time_seconds: 0.5,
            },
            SlideTiming {
                slide_index: 2,
                time_seconds: 1.0,
            },
        ];
        assert!(matches!(
            validate_timings(&timings),
            Err(ValidationError::InvalidSlideIndices)
        ));
    }

    #[test]
    fn request_overlay_width_range() {
        use crate::domain::{CompositionRequest, ForegroundKind, OverlayPosition};
        let req = CompositionRequest {
            pdf_path: "a.pdf".into(),
            video_path: "b.mp4".into(),
            output_path: "out.mp4".into(),
            overlay_position: OverlayPosition::TopRight,
            overlay_relative_width: 0.25,
            foreground_kind: ForegroundKind::Slides,
            quality: QualityProfile::Standard,
            fps: None,
            output_width: None,
            output_height: None,
            expected_duration_sec: None,
            timings: vec![
                SlideTiming {
                    slide_index: 0,
                    time_seconds: 0.1,
                },
                SlideTiming {
                    slide_index: 1,
                    time_seconds: 1.0,
                },
            ],
        };
        assert!(validate_request(&req).is_ok());

        let mut bad = req.clone();
        bad.overlay_relative_width = 0.0;
        assert!(matches!(
            validate_request(&bad),
            Err(ValidationError::OverlayWidthOutOfRange(_))
        ));
    }

    #[test]
    fn ffmpeg_args_build_basic() {
        let args = build_ffmpeg_args(
            "main.mp4",
            "overlay.mp4",
            "out.mp4",
            0.2,
            OverlayPosition::TopRight,
            ForegroundKind::Slides,
        );
        let joined = args.0.join(" ");
        assert!(joined.contains("-i main.mp4"));
        assert!(joined.contains("-i overlay.mp4"));
        assert!(joined.contains("filter_complex"));
        assert!(joined.contains("scale=1920*0.2"));
        assert!(joined.contains("-map 0:a?"));
        assert!(joined.contains("-shortest"));
        assert!(joined.contains("-loglevel warning"));
        assert!(joined.ends_with("out.mp4"));
    }

    #[test]
    fn images_to_video_args() {
        let args = build_images_to_video_args("frames/*.png", 30, "slides.mp4");
        let joined = args.0.join(" ");
        assert!(joined.contains("-framerate 30"));
        assert!(joined.contains("-pattern_type glob"));
        assert!(joined.contains("-i frames/*.png"));
        assert!(joined.contains("1920x1080"));
    }

    #[test]
    fn quality_profile_applies_flags() {
        let mut args = build_ffmpeg_args(
            "main.mp4",
            "overlay.mp4",
            "out.mp4",
            0.2,
            OverlayPosition::TopRight,
            ForegroundKind::Slides,
        );
        // Remember last token (output)
        let last_before = args.0.last().cloned().unwrap();
        apply_quality(&mut args, QualityProfile::Draft);
        let joined = args.0.join(" ");
        assert_eq!(args.0.last().unwrap(), &last_before);
        assert!(joined.contains("-crf 32"));
        assert!(joined.contains("-preset veryfast"));
    }
}
