use image::{DynamicImage, ImageFormat};
use serde::{Deserialize, Serialize};
use std::io::Cursor;

use crate::PipelineError;

/// Supported LLM providers with their image specifications
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum LlmProvider {
    Claude,
    #[serde(rename = "gpt")]
    Gpt4o,
    Gemini,
}

impl LlmProvider {
    /// Maximum long-edge resolution for optimal token usage
    pub fn max_long_edge(&self) -> u32 {
        match self {
            LlmProvider::Claude => 1568,
            LlmProvider::Gpt4o => 2048,
            LlmProvider::Gemini => 3072, // 4 tiles of 768
        }
    }

    /// Preferred output format
    pub fn preferred_format(&self) -> ImageFormat {
        match self {
            LlmProvider::Claude => ImageFormat::WebP,
            LlmProvider::Gpt4o => ImageFormat::Png,
            LlmProvider::Gemini => ImageFormat::WebP,
        }
    }

    /// Estimate token count for given dimensions
    pub fn estimate_tokens(&self, width: u32, height: u32) -> u32 {
        match self {
            LlmProvider::Claude => {
                // tokens = (width * height) / 750
                ((width as u64 * height as u64) / 750) as u32
            }
            LlmProvider::Gpt4o => {
                // High detail: ceil(w/512) * ceil(h/512) * 170 + 85
                let tiles_w = (width as f64 / 512.0).ceil() as u32;
                let tiles_h = (height as f64 / 512.0).ceil() as u32;
                tiles_w * tiles_h * 170 + 85
            }
            LlmProvider::Gemini => {
                if width <= 384 && height <= 384 {
                    258
                } else {
                    let tiles_w = (width as f64 / 768.0).ceil() as u32;
                    let tiles_h = (height as f64 / 768.0).ceil() as u32;
                    tiles_w * tiles_h * 258
                }
            }
        }
    }
}

/// Options for image optimization
#[derive(Debug, Clone)]
pub struct OptimizeOptions {
    pub provider: LlmProvider,
    pub max_tokens: Option<u32>,
    pub quality: u8, // 1-100, default 85
}

impl Default for OptimizeOptions {
    fn default() -> Self {
        Self {
            provider: LlmProvider::Claude,
            max_tokens: None,
            quality: 85,
        }
    }
}

/// Calculate optimal dimensions for a given provider
pub fn calculate_optimal_dimensions(width: u32, height: u32, provider: LlmProvider) -> (u32, u32) {
    let max_edge = provider.max_long_edge();
    let long_edge = width.max(height);

    if long_edge <= max_edge {
        return (width, height);
    }

    let scale = max_edge as f64 / long_edge as f64;
    let new_w = (width as f64 * scale).round() as u32;
    let new_h = (height as f64 * scale).round() as u32;
    (new_w.max(1), new_h.max(1))
}

/// Result of image optimization, containing both encoded bytes and raw RGBA.
pub struct OptimizeResult {
    /// Encoded image bytes (WebP/PNG) for LLM input.
    pub encoded: Vec<u8>,
    /// Raw RGBA pixel data of the resized image (for clipboard without re-decode).
    pub rgba: Vec<u8>,
    /// Width of the optimized image.
    pub width: u32,
    /// Height of the optimized image.
    pub height: u32,
}

/// Optimize an image for LLM input
///
/// Takes raw RGBA pixel data and returns encoded image bytes
/// in the optimal format for the specified provider, along with
/// the resized RGBA data for direct clipboard use.
pub fn optimize_image(
    rgba_data: &[u8],
    width: u32,
    height: u32,
    options: &OptimizeOptions,
) -> Result<OptimizeResult, PipelineError> {
    let img = DynamicImage::ImageRgba8(
        image::RgbaImage::from_raw(width, height, rgba_data.to_vec())
            .ok_or_else(|| PipelineError::InvalidImage("Invalid RGBA dimensions".into()))?,
    );

    // Calculate target dimensions
    let (target_w, target_h) =
        calculate_optimal_dimensions(img.width(), img.height(), options.provider);

    // Resize if needed
    let resized = if target_w != img.width() || target_h != img.height() {
        let _s = tracing::info_span!(
            "resize",
            from = %format!("{}x{}", img.width(), img.height()),
            to = %format!("{target_w}x{target_h}"),
        )
        .entered();
        img.resize_exact(target_w, target_h, image::imageops::FilterType::Lanczos3)
    } else {
        img
    };

    // Keep RGBA data before encoding (avoids re-decode for clipboard)
    let rgba_out = resized.to_rgba8().into_raw();

    // Encode in optimal format
    let format = options.provider.preferred_format();
    let encoded = {
        let _s = tracing::info_span!("encode", ?format).entered();
        let mut buf = Cursor::new(Vec::new());
        resized.write_to(&mut buf, format)?;
        buf.into_inner()
    };

    tracing::info!(
        provider = ?options.provider,
        output_size = encoded.len(),
        dimensions = %format!("{target_w}x{target_h}"),
        estimated_tokens = options.provider.estimate_tokens(target_w, target_h),
        "Image optimized for LLM"
    );

    Ok(OptimizeResult {
        encoded,
        rgba: rgba_out,
        width: target_w,
        height: target_h,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn claude_respects_max_long_edge() {
        let (w, h) = calculate_optimal_dimensions(3840, 2160, LlmProvider::Claude);
        assert!(w <= 1568, "width {w} exceeds 1568");
        assert!(h <= 1568, "height {h} exceeds 1568");
        // Aspect ratio should be roughly preserved
        let original_ratio = 3840.0 / 2160.0;
        let new_ratio = w as f64 / h as f64;
        assert!((original_ratio - new_ratio).abs() < 0.05);
    }

    #[test]
    fn small_image_not_upscaled() {
        let (w, h) = calculate_optimal_dimensions(800, 600, LlmProvider::Claude);
        assert_eq!(w, 800);
        assert_eq!(h, 600);
    }

    #[test]
    fn gpt4o_token_estimation() {
        // 1024x1024 image: ceil(1024/512) * ceil(1024/512) * 170 + 85 = 2*2*170+85 = 765
        let tokens = LlmProvider::Gpt4o.estimate_tokens(1024, 1024);
        assert_eq!(tokens, 765);
    }

    #[test]
    fn claude_token_estimation() {
        // 1000x1000: (1000*1000)/750 = 1333
        let tokens = LlmProvider::Claude.estimate_tokens(1000, 1000);
        assert_eq!(tokens, 1333);
    }

    #[test]
    fn gemini_small_image_flat_cost() {
        let tokens = LlmProvider::Gemini.estimate_tokens(200, 200);
        assert_eq!(tokens, 258);
    }

    #[test]
    fn gemini_large_image_tiled_cost() {
        // 1536x1536: ceil(1536/768)*ceil(1536/768)*258 = 2*2*258 = 1032
        let tokens = LlmProvider::Gemini.estimate_tokens(1536, 1536);
        assert_eq!(tokens, 1032);
    }

    #[test]
    fn optimize_produces_valid_output() {
        // Create a small 10x10 RGBA test image
        let width = 10u32;
        let height = 10u32;
        let rgba_data = vec![128u8; (width * height * 4) as usize];

        let options = OptimizeOptions {
            provider: LlmProvider::Claude,
            ..Default::default()
        };

        let result = optimize_image(&rgba_data, width, height, &options);
        assert!(result.is_ok(), "optimize_image failed: {:?}", result.err());
        let opt = result.unwrap();
        assert!(
            !opt.encoded.is_empty(),
            "Encoded output should not be empty"
        );
        assert_eq!(opt.width, width);
        assert_eq!(opt.height, height);
    }

    #[test]
    fn optimize_rejects_invalid_dimensions() {
        let rgba_data = vec![0u8; 100]; // Too small for 10x10
        let options = OptimizeOptions::default();

        let result = optimize_image(&rgba_data, 10, 10, &options);
        assert!(result.is_err());
    }

    // --- Edge case tests ---

    #[test]
    fn zero_width_no_resize_needed() {
        // 0x100: long_edge=100 <= 1568, so returns as-is (no downscale)
        let (w, h) = calculate_optimal_dimensions(0, 100, LlmProvider::Claude);
        assert_eq!((w, h), (0, 100));
    }

    #[test]
    fn zero_height_no_resize_needed() {
        // 100x0: long_edge=100 <= 1568, so returns as-is
        let (w, h) = calculate_optimal_dimensions(100, 0, LlmProvider::Claude);
        assert_eq!((w, h), (100, 0));
    }

    #[test]
    fn both_zero_dimensions() {
        let (w, h) = calculate_optimal_dimensions(0, 0, LlmProvider::Claude);
        assert_eq!((w, h), (0, 0));
    }

    #[test]
    fn zero_width_large_height_clamps_to_min_one() {
        // 0x5000: needs downscale. scale = 1568/5000 = 0.3136
        // new_w = round(0 * 0.3136) = 0, clamped to 1
        let (w, h) = calculate_optimal_dimensions(0, 5000, LlmProvider::Claude);
        assert_eq!(w, 1, "zero width clamped to 1 after downscale");
        assert!(h <= 1568);
    }

    #[test]
    fn zero_height_large_width_clamps_to_min_one() {
        let (w, h) = calculate_optimal_dimensions(5000, 0, LlmProvider::Claude);
        assert!(w <= 1568);
        assert_eq!(h, 1, "zero height clamped to 1 after downscale");
    }

    #[test]
    fn very_large_dimensions_claude() {
        let (w, h) = calculate_optimal_dimensions(10000, 5000, LlmProvider::Claude);
        assert!(w <= 1568, "width {w} exceeds max");
        assert!(h <= 1568, "height {h} exceeds max");
        assert!(w > 0);
        assert!(h > 0);
    }

    #[test]
    fn very_large_dimensions_gpt4o() {
        let (w, h) = calculate_optimal_dimensions(10000, 5000, LlmProvider::Gpt4o);
        assert!(w <= 2048, "width {w} exceeds max");
        assert!(h <= 2048, "height {h} exceeds max");
    }

    #[test]
    fn very_large_dimensions_gemini() {
        let (w, h) = calculate_optimal_dimensions(10000, 5000, LlmProvider::Gemini);
        assert!(w <= 3072, "width {w} exceeds max");
        assert!(h <= 3072, "height {h} exceeds max");
    }

    #[test]
    fn all_providers_preferred_format() {
        assert_eq!(LlmProvider::Claude.preferred_format(), ImageFormat::WebP);
        assert_eq!(LlmProvider::Gpt4o.preferred_format(), ImageFormat::Png);
        assert_eq!(LlmProvider::Gemini.preferred_format(), ImageFormat::WebP);
    }

    #[test]
    fn all_providers_max_long_edge() {
        assert_eq!(LlmProvider::Claude.max_long_edge(), 1568);
        assert_eq!(LlmProvider::Gpt4o.max_long_edge(), 2048);
        assert_eq!(LlmProvider::Gemini.max_long_edge(), 3072);
    }

    #[test]
    fn optimize_options_default_values() {
        let opts = OptimizeOptions::default();
        assert_eq!(opts.provider, LlmProvider::Claude);
        assert_eq!(opts.max_tokens, None);
        assert_eq!(opts.quality, 85);
    }

    #[test]
    fn landscape_orientation() {
        // Width > height: long edge is width
        let (w, h) = calculate_optimal_dimensions(3000, 1000, LlmProvider::Claude);
        assert_eq!(w, 1568); // Width capped to max
        assert!(h < w, "height should be less than width for landscape");
        // Check aspect ratio preservation
        let original_ratio = 3000.0 / 1000.0;
        let new_ratio = w as f64 / h as f64;
        assert!((original_ratio - new_ratio).abs() < 0.1);
    }

    #[test]
    fn portrait_orientation() {
        // Height > width: long edge is height
        let (w, h) = calculate_optimal_dimensions(1000, 3000, LlmProvider::Claude);
        assert_eq!(h, 1568); // Height capped to max
        assert!(w < h, "width should be less than height for portrait");
        let original_ratio = 1000.0 / 3000.0;
        let new_ratio = w as f64 / h as f64;
        assert!((original_ratio - new_ratio).abs() < 0.1);
    }

    #[test]
    fn square_image_at_boundary() {
        // Exactly at max edge — should not be resized
        let (w, h) = calculate_optimal_dimensions(1568, 1568, LlmProvider::Claude);
        assert_eq!((w, h), (1568, 1568));
    }

    #[test]
    fn one_pixel_over_boundary() {
        let (w, h) = calculate_optimal_dimensions(1569, 1569, LlmProvider::Claude);
        assert!(w <= 1568);
        assert!(h <= 1568);
    }

    #[test]
    fn gpt4o_single_tile_token_estimation() {
        // 512x512 = 1 tile = 1*1*170 + 85 = 255
        let tokens = LlmProvider::Gpt4o.estimate_tokens(512, 512);
        assert_eq!(tokens, 255);
    }

    #[test]
    fn gpt4o_tiny_image_token_estimation() {
        // 1x1: ceil(1/512)*ceil(1/512)*170 + 85 = 1*1*170+85 = 255
        let tokens = LlmProvider::Gpt4o.estimate_tokens(1, 1);
        assert_eq!(tokens, 255);
    }

    #[test]
    fn claude_zero_area_token_estimation() {
        let tokens = LlmProvider::Claude.estimate_tokens(0, 0);
        assert_eq!(tokens, 0);
    }

    #[test]
    fn gemini_boundary_small_image() {
        // Exactly 384x384 should still be flat cost
        let tokens = LlmProvider::Gemini.estimate_tokens(384, 384);
        assert_eq!(tokens, 258);
    }

    #[test]
    fn gemini_just_over_boundary() {
        // 385x384: exceeds small threshold, uses tiled
        let tokens = LlmProvider::Gemini.estimate_tokens(385, 384);
        // ceil(385/768)=1, ceil(384/768)=1 => 1*1*258 = 258
        assert_eq!(tokens, 258);
    }

    // --- LlmProvider deserialization tests ---

    #[test]
    fn deserialize_provider_claude() {
        let p: LlmProvider =
            serde_json::from_value(serde_json::Value::String("claude".into())).unwrap();
        assert_eq!(p, LlmProvider::Claude);
    }

    #[test]
    fn deserialize_provider_gpt() {
        let p: LlmProvider =
            serde_json::from_value(serde_json::Value::String("gpt".into())).unwrap();
        assert_eq!(p, LlmProvider::Gpt4o);
    }

    #[test]
    fn deserialize_provider_gemini() {
        let p: LlmProvider =
            serde_json::from_value(serde_json::Value::String("gemini".into())).unwrap();
        assert_eq!(p, LlmProvider::Gemini);
    }

    #[test]
    fn deserialize_provider_invalid_string_fails() {
        let result: Result<LlmProvider, _> =
            serde_json::from_value(serde_json::Value::String("unknown".into()));
        assert!(
            result.is_err(),
            "Unknown provider string should fail deserialization"
        );
    }

    #[test]
    fn serialize_provider_round_trip() {
        for provider in [LlmProvider::Claude, LlmProvider::Gpt4o, LlmProvider::Gemini] {
            let json = serde_json::to_value(provider).unwrap();
            let restored: LlmProvider = serde_json::from_value(json).unwrap();
            assert_eq!(provider, restored);
        }
    }

    #[test]
    fn serialize_provider_values() {
        assert_eq!(
            serde_json::to_value(LlmProvider::Claude).unwrap(),
            serde_json::Value::String("claude".into())
        );
        assert_eq!(
            serde_json::to_value(LlmProvider::Gpt4o).unwrap(),
            serde_json::Value::String("gpt".into())
        );
        assert_eq!(
            serde_json::to_value(LlmProvider::Gemini).unwrap(),
            serde_json::Value::String("gemini".into())
        );
    }

    #[test]
    fn provider_equality() {
        assert_eq!(LlmProvider::Claude, LlmProvider::Claude);
        assert_eq!(LlmProvider::Gpt4o, LlmProvider::Gpt4o);
        assert_eq!(LlmProvider::Gemini, LlmProvider::Gemini);
        assert_ne!(LlmProvider::Claude, LlmProvider::Gpt4o);
        assert_ne!(LlmProvider::Claude, LlmProvider::Gemini);
        assert_ne!(LlmProvider::Gpt4o, LlmProvider::Gemini);
    }

    mod proptests {
        use super::*;
        use proptest::prelude::*;

        proptest! {
            #[test]
            fn dimensions_never_exceed_provider_max(
                w in 1u32..20000,
                h in 1u32..20000
            ) {
                for provider in [LlmProvider::Claude, LlmProvider::Gpt4o, LlmProvider::Gemini] {
                    let (out_w, out_h) = calculate_optimal_dimensions(w, h, provider);
                    let max = provider.max_long_edge();
                    prop_assert!(out_w <= max, "width {} exceeds max {} for {:?} (input: {}x{})", out_w, max, provider, w, h);
                    prop_assert!(out_h <= max, "height {} exceeds max {} for {:?} (input: {}x{})", out_h, max, provider, w, h);
                }
            }

            #[test]
            fn dimensions_preserve_aspect_ratio(
                w in 100u32..10000,
                h in 100u32..10000
            ) {
                let (out_w, out_h) = calculate_optimal_dimensions(w, h, LlmProvider::Claude);
                if out_w < w || out_h < h {
                    // Only check when resizing actually happened
                    // Use relative error to handle extreme aspect ratios where
                    // rounding causes larger absolute differences
                    let original_ratio = w as f64 / h as f64;
                    let result_ratio = out_w as f64 / out_h as f64;
                    let relative_error = (original_ratio - result_ratio).abs() / original_ratio;
                    prop_assert!(
                        relative_error < 0.05,
                        "Aspect ratio changed: {:.3} -> {:.3} (rel err {:.4}) ({}x{} -> {}x{})",
                        original_ratio, result_ratio, relative_error, w, h, out_w, out_h
                    );
                }
            }

            #[test]
            fn dimensions_never_upscale(
                w in 1u32..5000,
                h in 1u32..5000
            ) {
                let (out_w, out_h) = calculate_optimal_dimensions(w, h, LlmProvider::Claude);
                prop_assert!(out_w <= w, "width upscaled: {} -> {}", w, out_w);
                prop_assert!(out_h <= h, "height upscaled: {} -> {}", h, out_h);
            }

            #[test]
            fn dimensions_are_nonzero(
                w in 1u32..20000,
                h in 1u32..20000
            ) {
                let (out_w, out_h) = calculate_optimal_dimensions(w, h, LlmProvider::Claude);
                prop_assert!(out_w > 0);
                prop_assert!(out_h > 0);
            }

            #[test]
            fn token_estimate_is_positive(
                w in 28u32..10000,
                h in 28u32..10000
            ) {
                for provider in [LlmProvider::Claude, LlmProvider::Gpt4o, LlmProvider::Gemini] {
                    let tokens = provider.estimate_tokens(w, h);
                    prop_assert!(tokens > 0, "Zero tokens for {:?} at {}x{}", provider, w, h);
                }
            }
        }
    }
}
