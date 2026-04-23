use image::{DynamicImage, ImageFormat};
use std::io::Cursor;

use crate::PipelineError;

/// Universal optimal long-edge for LLM input. Fits within Claude (8000), GPT-4o (2048),
/// and Gemini limits while preserving readable text in typical UI screenshots.
pub const UNIVERSAL_MAX_LONG_EDGE: u32 = 1568;

/// Universal output format for LLM-bound images.
pub const UNIVERSAL_OUTPUT_FORMAT: ImageFormat = ImageFormat::WebP;

/// Default WebP quality (1-100). 85 keeps text legible at typical screenshot sizes.
pub const DEFAULT_QUALITY: u8 = 85;

/// Options for image optimization.
#[derive(Debug, Clone)]
pub struct OptimizeOptions {
    pub quality: u8,
}

impl Default for OptimizeOptions {
    fn default() -> Self {
        Self {
            quality: DEFAULT_QUALITY,
        }
    }
}

/// Calculate optimal dimensions, downscaling so the long edge fits
/// `UNIVERSAL_MAX_LONG_EDGE`. Never upscales. Clamps zero outputs to 1.
pub fn calculate_optimal_dimensions(width: u32, height: u32) -> (u32, u32) {
    let long_edge = width.max(height);

    if long_edge <= UNIVERSAL_MAX_LONG_EDGE {
        return (width, height);
    }

    let scale = UNIVERSAL_MAX_LONG_EDGE as f64 / long_edge as f64;
    let new_w = (width as f64 * scale).round() as u32;
    let new_h = (height as f64 * scale).round() as u32;
    (new_w.max(1), new_h.max(1))
}

/// Result of image optimization, containing both encoded bytes and raw RGBA.
pub struct OptimizeResult {
    /// Encoded image bytes (WebP) for LLM input.
    pub encoded: Vec<u8>,
    /// Raw RGBA pixel data of the resized image (for clipboard without re-decode).
    pub rgba: Vec<u8>,
    /// Width of the optimized image.
    pub width: u32,
    /// Height of the optimized image.
    pub height: u32,
}

/// Optimize an image for LLM input.
///
/// Takes raw RGBA pixel data and returns encoded WebP bytes
/// resized to a universal long-edge cap that suits all major LLM providers,
/// along with the resized RGBA data for direct clipboard use.
pub fn optimize_image(
    rgba_data: &[u8],
    width: u32,
    height: u32,
    _options: &OptimizeOptions,
) -> Result<OptimizeResult, PipelineError> {
    let img = DynamicImage::ImageRgba8(
        image::RgbaImage::from_raw(width, height, rgba_data.to_vec())
            .ok_or_else(|| PipelineError::InvalidImage("Invalid RGBA dimensions".into()))?,
    );

    let (target_w, target_h) = calculate_optimal_dimensions(img.width(), img.height());

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

    let rgba_out = resized.to_rgba8().into_raw();

    let encoded = {
        let _s = tracing::info_span!("encode", format = ?UNIVERSAL_OUTPUT_FORMAT).entered();
        let mut buf = Cursor::new(Vec::new());
        resized.write_to(&mut buf, UNIVERSAL_OUTPUT_FORMAT)?;
        buf.into_inner()
    };

    tracing::info!(
        output_size = encoded.len(),
        dimensions = %format!("{target_w}x{target_h}"),
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
    fn respects_max_long_edge() {
        let (w, h) = calculate_optimal_dimensions(3840, 2160);
        assert!(w <= UNIVERSAL_MAX_LONG_EDGE, "width {w} exceeds cap");
        assert!(h <= UNIVERSAL_MAX_LONG_EDGE, "height {h} exceeds cap");
        let original_ratio = 3840.0 / 2160.0;
        let new_ratio = w as f64 / h as f64;
        assert!((original_ratio - new_ratio).abs() < 0.05);
    }

    #[test]
    fn small_image_not_upscaled() {
        let (w, h) = calculate_optimal_dimensions(800, 600);
        assert_eq!(w, 800);
        assert_eq!(h, 600);
    }

    #[test]
    fn optimize_produces_valid_output() {
        let width = 10u32;
        let height = 10u32;
        let rgba_data = vec![128u8; (width * height * 4) as usize];

        let result = optimize_image(&rgba_data, width, height, &OptimizeOptions::default());
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
        let rgba_data = vec![0u8; 100];
        let result = optimize_image(&rgba_data, 10, 10, &OptimizeOptions::default());
        assert!(result.is_err());
    }

    #[test]
    fn zero_width_no_resize_needed() {
        let (w, h) = calculate_optimal_dimensions(0, 100);
        assert_eq!((w, h), (0, 100));
    }

    #[test]
    fn zero_height_no_resize_needed() {
        let (w, h) = calculate_optimal_dimensions(100, 0);
        assert_eq!((w, h), (100, 0));
    }

    #[test]
    fn both_zero_dimensions() {
        let (w, h) = calculate_optimal_dimensions(0, 0);
        assert_eq!((w, h), (0, 0));
    }

    #[test]
    fn zero_width_large_height_clamps_to_min_one() {
        let (w, h) = calculate_optimal_dimensions(0, 5000);
        assert_eq!(w, 1, "zero width clamped to 1 after downscale");
        assert!(h <= UNIVERSAL_MAX_LONG_EDGE);
    }

    #[test]
    fn zero_height_large_width_clamps_to_min_one() {
        let (w, h) = calculate_optimal_dimensions(5000, 0);
        assert!(w <= UNIVERSAL_MAX_LONG_EDGE);
        assert_eq!(h, 1, "zero height clamped to 1 after downscale");
    }

    #[test]
    fn very_large_dimensions() {
        let (w, h) = calculate_optimal_dimensions(10000, 5000);
        assert!(w <= UNIVERSAL_MAX_LONG_EDGE);
        assert!(h <= UNIVERSAL_MAX_LONG_EDGE);
        assert!(w > 0);
        assert!(h > 0);
    }

    #[test]
    fn optimize_options_default_values() {
        let opts = OptimizeOptions::default();
        assert_eq!(opts.quality, DEFAULT_QUALITY);
    }

    #[test]
    fn landscape_orientation() {
        let (w, h) = calculate_optimal_dimensions(3000, 1000);
        assert_eq!(w, UNIVERSAL_MAX_LONG_EDGE);
        assert!(h < w);
        let original_ratio = 3000.0 / 1000.0;
        let new_ratio = w as f64 / h as f64;
        assert!((original_ratio - new_ratio).abs() < 0.1);
    }

    #[test]
    fn portrait_orientation() {
        let (w, h) = calculate_optimal_dimensions(1000, 3000);
        assert_eq!(h, UNIVERSAL_MAX_LONG_EDGE);
        assert!(w < h);
        let original_ratio = 1000.0 / 3000.0;
        let new_ratio = w as f64 / h as f64;
        assert!((original_ratio - new_ratio).abs() < 0.1);
    }

    #[test]
    fn square_image_at_boundary() {
        let (w, h) = calculate_optimal_dimensions(UNIVERSAL_MAX_LONG_EDGE, UNIVERSAL_MAX_LONG_EDGE);
        assert_eq!((w, h), (UNIVERSAL_MAX_LONG_EDGE, UNIVERSAL_MAX_LONG_EDGE));
    }

    #[test]
    fn one_pixel_over_boundary() {
        let (w, h) =
            calculate_optimal_dimensions(UNIVERSAL_MAX_LONG_EDGE + 1, UNIVERSAL_MAX_LONG_EDGE + 1);
        assert!(w <= UNIVERSAL_MAX_LONG_EDGE);
        assert!(h <= UNIVERSAL_MAX_LONG_EDGE);
    }

    mod proptests {
        use super::*;
        use proptest::prelude::*;

        proptest! {
            #[test]
            fn dimensions_never_exceed_max(
                w in 1u32..20000,
                h in 1u32..20000
            ) {
                let (out_w, out_h) = calculate_optimal_dimensions(w, h);
                prop_assert!(out_w <= UNIVERSAL_MAX_LONG_EDGE);
                prop_assert!(out_h <= UNIVERSAL_MAX_LONG_EDGE);
            }

            #[test]
            fn dimensions_preserve_aspect_ratio(
                w in 100u32..10000,
                h in 100u32..10000
            ) {
                let (out_w, out_h) = calculate_optimal_dimensions(w, h);
                if out_w < w || out_h < h {
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
                let (out_w, out_h) = calculate_optimal_dimensions(w, h);
                prop_assert!(out_w <= w);
                prop_assert!(out_h <= h);
            }

            #[test]
            fn dimensions_are_nonzero(
                w in 1u32..20000,
                h in 1u32..20000
            ) {
                let (out_w, out_h) = calculate_optimal_dimensions(w, h);
                prop_assert!(out_w > 0);
                prop_assert!(out_h > 0);
            }
        }
    }
}
