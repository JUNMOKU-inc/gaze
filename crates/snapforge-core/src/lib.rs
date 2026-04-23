mod annotation;
mod capture;
mod output;
mod settings;

pub use annotation::{
    apply_annotations_to_processed_capture, build_llm_prompt_hint,
    build_llm_prompt_hint_for_language, render_annotations_to_png, AnnotatedImage, Annotation,
    AnnotationKind,
};
pub use capture::{
    process_image_bytes, process_image_bytes_with_mode, process_rgba_capture,
    process_rgba_capture_with_mode, temp_capture_path, CaptureMetadata, CaptureProcessingMode,
    ProcessedCapture,
};
pub use output::{build_capture_filename, detect_image_format};
pub use settings::{
    default_settings_path, get_setting_field, load_settings, load_settings_or_default,
    save_settings, set_setting_field, settings_path_for_identifier, MaxDimension, Settings,
    SettingsError, GAZE_BUNDLE_IDENTIFIER, SETTINGS_KEYS,
};

use thiserror::Error;

#[derive(Debug, Error)]
pub enum CaptureFlowError {
    #[error("Failed to guess image format: {0}")]
    GuessImageFormat(#[source] std::io::Error),

    #[error("Failed to decode captured image: {0}")]
    DecodeImage(#[source] image::ImageError),

    #[error("Optimization failed: {0}")]
    Optimize(#[from] snapforge_pipeline::PipelineError),

    #[error("Invalid RGBA buffer for {width}x{height} image")]
    InvalidRgbaDimensions { width: u32, height: u32 },

    #[error("Failed to encode image: {0}")]
    EncodeImage(#[source] image::ImageError),
}
