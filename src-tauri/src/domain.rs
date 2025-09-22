use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum OverlayPosition {
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum ForegroundKind {
    Slides,
    Video,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum QualityProfile {
    Draft,
    Standard,
    High,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SlideTiming {
    pub slide_index: u32,
    pub time_seconds: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CompositionRequest {
    pub pdf_path: String,
    pub video_path: String,
    pub output_path: String,
    pub overlay_position: OverlayPosition,
    /// 0.05..=0.50 of 1920
    pub overlay_relative_width: f64,
    /// What to display in the overlay (default: Slides)
    pub foreground_kind: ForegroundKind,
    /// Encoding quality/speed
    pub quality: QualityProfile,
    /// output frames per second
    pub fps: Option<u32>,
    /// output width x height; if None, inherit from main video
    pub output_width: Option<u32>,
    pub output_height: Option<u32>,
    /// expected duration for progress (seconds)
    pub expected_duration_sec: Option<f64>,
    /// slide switch times; must be sorted by time_seconds ascending and cover all slides
    pub timings: Vec<SlideTiming>,
}

#[derive(thiserror::Error, Debug, PartialEq)]
pub enum ValidationError {
    #[error("overlay_relative_width must be within [0.05, 0.25], got {0}")]
    OverlayWidthOutOfRange(f64),
    #[error("timings must not be empty")]
    EmptyTimings,
    #[error("timings must be strictly increasing by time")]
    NonIncreasingTimings,
    #[error("slide indices must start at 0 and be contiguous")]
    InvalidSlideIndices,
}
