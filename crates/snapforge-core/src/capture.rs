use base64::Engine as _;
use image::{DynamicImage, ImageReader};
use serde::{Deserialize, Serialize};
use snapforge_pipeline::{optimize_image, LlmProvider, OptimizeOptions};
use std::io::Cursor;
use std::path::PathBuf;

use crate::CaptureFlowError;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CaptureMetadata {
    pub original_width: u32,
    pub original_height: u32,
    pub optimized_width: u32,
    pub optimized_height: u32,
    pub file_size: usize,
    pub token_estimate: u32,
    pub provider: String,
    pub timestamp: String,
    pub image_base64: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcessedCapture {
    pub metadata: CaptureMetadata,
    pub encoded: Vec<u8>,
    pub rgba: Vec<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaptureProcessingMode {
    Optimized,
    Raw,
}

pub fn temp_capture_path() -> PathBuf {
    std::env::temp_dir().join(format!(
        "gaze_{}.png",
        chrono::Local::now().format("%Y%m%d_%H%M%S_%3f")
    ))
}

pub fn process_image_bytes(
    raw_bytes: &[u8],
    provider: LlmProvider,
) -> Result<ProcessedCapture, CaptureFlowError> {
    process_image_bytes_with_mode(raw_bytes, provider, CaptureProcessingMode::Optimized)
}

pub fn process_image_bytes_with_mode(
    raw_bytes: &[u8],
    provider: LlmProvider,
    mode: CaptureProcessingMode,
) -> Result<ProcessedCapture, CaptureFlowError> {
    let _root = tracing::info_span!("process_image_bytes", ?provider, ?mode).entered();

    let img = ImageReader::new(Cursor::new(raw_bytes))
        .with_guessed_format()
        .map_err(CaptureFlowError::GuessImageFormat)?
        .decode()
        .map_err(CaptureFlowError::DecodeImage)?;

    let rgba = img.to_rgba8();
    let (width, height) = rgba.dimensions();
    process_rgba_capture_with_mode(&rgba.into_raw(), width, height, provider, mode)
}

pub fn process_rgba_capture(
    rgba_data: &[u8],
    width: u32,
    height: u32,
    provider: LlmProvider,
) -> Result<ProcessedCapture, CaptureFlowError> {
    process_rgba_capture_with_mode(
        rgba_data,
        width,
        height,
        provider,
        CaptureProcessingMode::Optimized,
    )
}

pub fn process_rgba_capture_with_mode(
    rgba_data: &[u8],
    width: u32,
    height: u32,
    provider: LlmProvider,
    mode: CaptureProcessingMode,
) -> Result<ProcessedCapture, CaptureFlowError> {
    let _root =
        tracing::info_span!("process_rgba_capture", ?provider, ?mode, width, height).entered();

    let (encoded, rgba, optimized_width, optimized_height) = match mode {
        CaptureProcessingMode::Optimized => {
            let options = OptimizeOptions {
                provider,
                ..Default::default()
            };
            let optimized = optimize_image(rgba_data, width, height, &options)?;
            (
                optimized.encoded,
                optimized.rgba,
                optimized.width,
                optimized.height,
            )
        }
        CaptureProcessingMode::Raw => {
            let encoded = encode_png(rgba_data, width, height)?;
            (encoded, rgba_data.to_vec(), width, height)
        }
    };

    let file_size = encoded.len();
    let image_base64 = base64::engine::general_purpose::STANDARD.encode(&encoded);
    let metadata = CaptureMetadata {
        original_width: width,
        original_height: height,
        optimized_width,
        optimized_height,
        file_size,
        token_estimate: provider.estimate_tokens(optimized_width, optimized_height),
        provider: format!("{provider:?}"),
        timestamp: chrono::Local::now().to_rfc3339(),
        image_base64,
    };

    Ok(ProcessedCapture {
        metadata,
        encoded,
        rgba,
    })
}

fn encode_png(rgba_data: &[u8], width: u32, height: u32) -> Result<Vec<u8>, CaptureFlowError> {
    let image = image::RgbaImage::from_raw(width, height, rgba_data.to_vec())
        .ok_or(CaptureFlowError::InvalidRgbaDimensions { width, height })?;

    let mut cursor = Cursor::new(Vec::new());
    DynamicImage::ImageRgba8(image)
        .write_to(&mut cursor, image::ImageFormat::Png)
        .map_err(CaptureFlowError::EncodeImage)?;

    Ok(cursor.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::detect_image_format;

    fn create_test_png(width: u32, height: u32) -> Vec<u8> {
        use image::{ImageBuffer, Rgba};

        let img = ImageBuffer::from_fn(width, height, |_, _| Rgba([255u8, 0, 0, 255]));
        let mut cursor = Cursor::new(Vec::new());
        DynamicImage::ImageRgba8(img)
            .write_to(&mut cursor, image::ImageFormat::Png)
            .unwrap();
        cursor.into_inner()
    }

    fn sample_metadata() -> CaptureMetadata {
        CaptureMetadata {
            original_width: 1920,
            original_height: 1080,
            optimized_width: 1568,
            optimized_height: 882,
            file_size: 42_000,
            token_estimate: 1827,
            provider: "Claude".to_string(),
            timestamp: "2026-03-27T12:00:00+09:00".to_string(),
            image_base64: "iVBOR...".to_string(),
        }
    }

    #[test]
    fn metadata_camel_case_serialization() {
        let json = serde_json::to_string(&sample_metadata()).unwrap();

        assert!(json.contains("\"originalWidth\""));
        assert!(json.contains("\"optimizedHeight\""));
        assert!(json.contains("\"fileSize\""));
        assert!(json.contains("\"tokenEstimate\""));
        assert!(json.contains("\"imageBase64\""));
        assert!(!json.contains("\"original_width\""));
    }

    #[test]
    fn metadata_json_round_trip() {
        let original = sample_metadata();
        let json = serde_json::to_string(&original).unwrap();
        let restored: CaptureMetadata = serde_json::from_str(&json).unwrap();
        assert_eq!(original, restored);
    }

    #[test]
    fn metadata_deserialization_from_frontend_json() {
        let json = r#"{
            "originalWidth": 800,
            "originalHeight": 600,
            "optimizedWidth": 784,
            "optimizedHeight": 588,
            "fileSize": 15000,
            "tokenEstimate": 615,
            "provider": "Gpt4o",
            "timestamp": "2026-03-27T12:00:00Z",
            "imageBase64": "AAAA"
        }"#;
        let restored: CaptureMetadata = serde_json::from_str(json).unwrap();
        assert_eq!(restored.original_width, 800);
        assert_eq!(restored.optimized_height, 588);
        assert_eq!(restored.provider, "Gpt4o");
    }

    #[test]
    fn temp_capture_path_is_in_temp_dir_with_png_extension() {
        let path = temp_capture_path();
        assert!(path.starts_with(std::env::temp_dir()));
        assert_eq!(path.extension().and_then(|ext| ext.to_str()), Some("png"));
        assert!(path
            .file_name()
            .unwrap()
            .to_string_lossy()
            .starts_with("gaze_"));
    }

    #[test]
    fn temp_capture_path_is_unique() {
        let p1 = temp_capture_path();
        std::thread::sleep(std::time::Duration::from_millis(2));
        let p2 = temp_capture_path();
        assert_ne!(p1, p2);
    }

    #[test]
    fn process_image_bytes_valid_png() {
        let png = create_test_png(100, 50);
        let processed = process_image_bytes(&png, LlmProvider::Claude).unwrap();
        assert_eq!(processed.metadata.original_width, 100);
        assert_eq!(processed.metadata.original_height, 50);
        assert_eq!(processed.metadata.provider, "Claude");
        assert!(!processed.encoded.is_empty());
        assert!(!processed.rgba.is_empty());
    }

    #[test]
    fn process_image_bytes_large_image_resized() {
        let png = create_test_png(4000, 3000);
        let processed = process_image_bytes(&png, LlmProvider::Claude).unwrap();
        assert!(processed.metadata.optimized_width <= 1568);
        assert!(processed.metadata.optimized_height <= 1568);
        assert!(
            processed.metadata.optimized_width < processed.metadata.original_width
                || processed.metadata.optimized_height < processed.metadata.original_height
        );
    }

    #[test]
    fn process_image_bytes_small_image_not_upscaled() {
        let png = create_test_png(50, 30);
        let processed = process_image_bytes(&png, LlmProvider::Claude).unwrap();
        assert_eq!(processed.metadata.optimized_width, 50);
        assert_eq!(processed.metadata.optimized_height, 30);
    }

    #[test]
    fn process_image_bytes_invalid_data() {
        let result = process_image_bytes(&[0, 1, 2, 3], LlmProvider::Claude);
        assert!(matches!(result, Err(CaptureFlowError::DecodeImage(_))));
    }

    #[test]
    fn process_image_bytes_empty_data() {
        let result = process_image_bytes(&[], LlmProvider::Claude);
        assert!(result.is_err());
    }

    #[test]
    fn process_image_bytes_base64_is_valid() {
        let png = create_test_png(100, 100);
        let processed = process_image_bytes(&png, LlmProvider::Claude).unwrap();
        let decoded = base64::engine::general_purpose::STANDARD
            .decode(&processed.metadata.image_base64)
            .unwrap();
        assert_eq!(decoded, processed.encoded);
    }

    #[test]
    fn process_image_bytes_metadata_has_timestamp() {
        let png = create_test_png(100, 100);
        let processed = process_image_bytes(&png, LlmProvider::Claude).unwrap();
        assert!(processed.metadata.timestamp.contains('T'));
    }

    #[test]
    fn process_rgba_capture_supports_all_providers() {
        let rgba = vec![128u8; 200 * 150 * 4];
        for provider in [LlmProvider::Claude, LlmProvider::Gpt4o, LlmProvider::Gemini] {
            let processed = process_rgba_capture(&rgba, 200, 150, provider).unwrap();
            assert!(processed.metadata.optimized_width > 0);
            assert!(processed.metadata.optimized_height > 0);
            assert!(processed.metadata.token_estimate > 0);
        }
    }

    #[test]
    fn process_rgba_capture_raw_preserves_dimensions_and_encodes_png() {
        let rgba = vec![255u8; 25 * 10 * 4];
        let processed = process_rgba_capture_with_mode(
            &rgba,
            25,
            10,
            LlmProvider::Claude,
            CaptureProcessingMode::Raw,
        )
        .unwrap();

        assert_eq!(processed.metadata.original_width, 25);
        assert_eq!(processed.metadata.optimized_width, 25);
        assert_eq!(processed.rgba, rgba);
        assert_eq!(detect_image_format(&processed.encoded), "png");
    }

    #[test]
    fn process_rgba_capture_invalid_dimensions_fail() {
        let result = process_rgba_capture_with_mode(
            &[0u8; 10],
            10,
            10,
            LlmProvider::Claude,
            CaptureProcessingMode::Raw,
        );

        assert!(matches!(
            result,
            Err(CaptureFlowError::InvalidRgbaDimensions {
                width: 10,
                height: 10,
            })
        ));
    }
}
