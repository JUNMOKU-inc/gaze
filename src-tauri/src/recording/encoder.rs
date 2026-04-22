use snapforge_capture::FrameData;
use std::path::Path;

/// Encode a sequence of frames to a GIF file using gifski.
///
/// This runs gifski's multi-threaded pipeline:
/// - Thread 1 (collector): feeds frames to gifski
/// - Thread 2 (writer): writes the GIF output
pub fn encode_gif(
    frames: &[FrameData],
    output_path: &Path,
    _fps: u32,
    quality: u8,
) -> Result<(), String> {
    if frames.is_empty() {
        return Err("No frames to encode".to_string());
    }

    let first = &frames[0];

    let settings = gifski::Settings {
        width: Some(first.width),
        height: Some(first.height),
        quality,
        fast: false,
        repeat: gifski::Repeat::Infinite,
    };

    let (collector, writer) =
        gifski::new(settings).map_err(|e| format!("Failed to create gifski encoder: {e}"))?;

    // Clone frames data for the collector thread
    let frames_owned: Vec<(usize, Vec<u8>, u32, u32, f64)> = frames
        .iter()
        .enumerate()
        .map(|(i, f)| (i, f.rgba.clone(), f.width, f.height, f.timestamp_secs))
        .collect();

    // Gifski requires collector and writer to run on separate threads
    std::thread::scope(|scope| -> Result<(), String> {
        // Feed frames to gifski collector
        let feed_thread = scope.spawn(move || -> Result<(), String> {
            for (index, rgba, width, height, timestamp) in &frames_owned {
                // gifski requires ImgVec<RGBA8> (owned Vec), not a slice reference
                let pixels: Vec<rgb::RGBA8> = rgba
                    .chunks_exact(4)
                    .map(|c| rgb::RGBA8::new(c[0], c[1], c[2], c[3]))
                    .collect();
                let img = imgref::Img::new(pixels, *width as usize, *height as usize);
                collector
                    .add_frame_rgba(*index, img, *timestamp)
                    .map_err(|e| format!("Failed to add frame {index}: {e}"))?;
            }
            // Drop collector to signal end of input
            drop(collector);
            Ok(())
        });

        // Write GIF output
        let file = std::fs::File::create(output_path)
            .map_err(|e| format!("Failed to create GIF file: {e}"))?;

        writer
            .write(file, &mut gifski::progress::NoProgress {})
            .map_err(|e| format!("Failed to write GIF: {e}"))?;

        // Wait for feed thread
        feed_thread
            .join()
            .map_err(|_| "Feed thread panicked".to_string())??;

        Ok(())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_frames(count: usize, width: u32, height: u32) -> Vec<FrameData> {
        (0..count)
            .map(|i| {
                // Create frames with different colors
                let r = ((i * 50) % 256) as u8;
                let g = ((i * 30 + 100) % 256) as u8;
                let b = ((i * 70 + 50) % 256) as u8;
                let pixel_count = (width * height) as usize;
                let mut rgba = Vec::with_capacity(pixel_count * 4);
                for _ in 0..pixel_count {
                    rgba.extend_from_slice(&[r, g, b, 255]);
                }
                FrameData {
                    rgba,
                    width,
                    height,
                    timestamp_secs: i as f64 / 10.0,
                }
            })
            .collect()
    }

    #[test]
    fn encode_gif_empty_frames() {
        let result = encode_gif(&[], Path::new("/tmp/test.gif"), 10, 80);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("No frames"));
    }

    #[test]
    fn encode_gif_single_frame() {
        let frames = create_test_frames(1, 20, 20);
        let path = std::env::temp_dir().join("gaze_test_single_frame.gif");
        let result = encode_gif(&frames, &path, 10, 80);
        let _ = std::fs::remove_file(&path);
        assert!(result.is_ok(), "encode_gif failed: {:?}", result.err());
    }

    #[test]
    fn encode_gif_multiple_frames() {
        let frames = create_test_frames(5, 32, 32);
        let path = std::env::temp_dir().join("gaze_test_multi_frame.gif");
        let result = encode_gif(&frames, &path, 10, 80);

        assert!(result.is_ok(), "encode_gif failed: {:?}", result.err());

        // Verify the GIF file was created and has content
        let metadata = std::fs::metadata(&path).expect("GIF file should exist");
        assert!(metadata.len() > 0, "GIF file should not be empty");

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn encode_gif_quality_range() {
        let frames = create_test_frames(3, 16, 16);

        for quality in [1, 50, 100] {
            let path = std::env::temp_dir().join(format!("gaze_test_quality_{quality}.gif"));
            let result = encode_gif(&frames, &path, 10, quality);
            let _ = std::fs::remove_file(&path);
            assert!(
                result.is_ok(),
                "quality {quality} failed: {:?}",
                result.err()
            );
        }
    }
}
