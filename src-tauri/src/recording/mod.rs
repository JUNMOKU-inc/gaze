mod encoder;

use snapforge_capture::{FrameData, StreamConfig};
use std::path::PathBuf;
use std::sync::mpsc;
use std::time::Instant;

/// Recording configuration
#[derive(Debug, Clone)]
pub struct RecordingConfig {
    pub display_id: u32,
    pub fps: u32,
    pub max_width: u32,
    pub max_duration_secs: u32,
    pub quality: u8,
    pub show_cursor: bool,
}

impl Default for RecordingConfig {
    fn default() -> Self {
        Self {
            display_id: 1,
            fps: 15,
            max_width: 640,
            max_duration_secs: 30,
            quality: 90,
            show_cursor: true,
        }
    }
}

/// Handle for a recording in progress
pub struct RecordingHandle {
    stop_stream: Option<Box<dyn FnOnce() + Send>>,
    collector_thread: Option<std::thread::JoinHandle<Vec<FrameData>>>,
    config: RecordingConfig,
}

/// Result of a completed recording
#[derive(Debug, Clone)]
pub struct RecordingResult {
    pub gif_path: PathBuf,
    pub file_size: u64,
    pub frame_count: u32,
    pub duration_secs: f64,
    pub width: u32,
    pub height: u32,
}

/// Start a new GIF recording session.
///
/// Returns a handle that can be used to stop the recording.
/// The recording captures frames from the main display at the configured FPS.
pub fn start_recording(config: RecordingConfig) -> Result<RecordingHandle, String> {
    // Get display dimensions for stream config
    let engine = snapforge_capture::create_engine();
    let displays = engine
        .list_displays()
        .map_err(|e| format!("Failed to list displays: {e}"))?;

    let display = displays
        .iter()
        .find(|d| d.id == config.display_id)
        .or_else(|| displays.first())
        .ok_or("No displays found")?;

    // Calculate capture dimensions: maintain aspect ratio, limit to max_width
    let (capture_width, capture_height) = if display.width > config.max_width {
        let scale = f64::from(config.max_width) / f64::from(display.width);
        let h = (f64::from(display.height) * scale) as u32;
        // Ensure even dimensions (required by some encoders)
        (config.max_width & !1, h & !1)
    } else {
        (display.width & !1, display.height & !1)
    };

    let stream_config = StreamConfig {
        display_id: display.id,
        fps: config.fps,
        width: capture_width,
        height: capture_height,
        show_cursor: config.show_cursor,
    };

    tracing::info!(
        fps = config.fps,
        width = capture_width,
        height = capture_height,
        max_duration = config.max_duration_secs,
        "Starting GIF recording"
    );

    let (frame_rx, stop_fn) = snapforge_capture::start_recording_stream(stream_config)
        .map_err(|e| format!("Failed to start recording stream: {e}"))?;

    // Spawn a thread to collect frames (with max duration enforcement)
    let max_duration = config.max_duration_secs;
    let collector_thread = std::thread::spawn(move || collect_frames(frame_rx, max_duration));

    Ok(RecordingHandle {
        stop_stream: Some(stop_fn),
        collector_thread: Some(collector_thread),
        config,
    })
}

/// Collect frames from the stream receiver until the channel closes or max duration is reached.
fn collect_frames(rx: mpsc::Receiver<FrameData>, max_duration_secs: u32) -> Vec<FrameData> {
    let mut frames = Vec::new();
    let start = Instant::now();
    let max_duration = std::time::Duration::from_secs(u64::from(max_duration_secs));

    loop {
        // Use a timeout to periodically check duration
        match rx.recv_timeout(std::time::Duration::from_millis(100)) {
            Ok(frame) => {
                frames.push(frame);
                if start.elapsed() >= max_duration {
                    tracing::info!(
                        frames = frames.len(),
                        "Max recording duration reached, stopping collection"
                    );
                    break;
                }
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {
                if start.elapsed() >= max_duration {
                    tracing::info!("Max recording duration reached");
                    break;
                }
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                tracing::debug!(frames = frames.len(), "Stream ended, collected all frames");
                break;
            }
        }
    }

    frames
}

/// Stop a recording and encode the captured frames to GIF.
///
/// This stops the capture stream, waits for all frames to be collected,
/// then encodes them to a GIF file using gifski.
pub fn stop_recording(mut handle: RecordingHandle) -> Result<RecordingResult, String> {
    // 1. Stop the capture stream
    if let Some(stop_fn) = handle.stop_stream.take() {
        tracing::info!("Stopping capture stream");
        stop_fn();
    }

    // 2. Wait for the collector thread to finish
    let frames = handle
        .collector_thread
        .take()
        .ok_or("Collector thread already consumed")?
        .join()
        .map_err(|_| "Collector thread panicked")?;

    if frames.is_empty() {
        return Err("No frames captured".to_string());
    }

    let frame_count = frames.len() as u32;
    let first_frame = &frames[0];
    let width = first_frame.width;
    let height = first_frame.height;
    let duration_secs = frames.last().map(|f| f.timestamp_secs).unwrap_or(0.0);

    tracing::info!(
        frame_count,
        width,
        height,
        duration_secs,
        "Encoding frames to GIF"
    );

    // 3. Generate output path
    let gif_path = std::env::temp_dir().join(format!(
        "gaze_recording_{}.gif",
        chrono::Local::now().format("%Y%m%d_%H%M%S")
    ));

    // 4. Encode to GIF
    encoder::encode_gif(&frames, &gif_path, handle.config.fps, handle.config.quality)?;

    let file_size = std::fs::metadata(&gif_path).map(|m| m.len()).unwrap_or(0);

    tracing::info!(
        path = %gif_path.display(),
        file_size,
        "GIF recording complete"
    );

    Ok(RecordingResult {
        gif_path,
        file_size,
        frame_count,
        duration_secs,
        width,
        height,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_values() {
        let config = RecordingConfig::default();
        assert_eq!(config.fps, 15);
        assert_eq!(config.max_width, 640);
        assert_eq!(config.max_duration_secs, 30);
        assert_eq!(config.quality, 90);
        assert!(config.show_cursor);
    }

    #[test]
    fn collect_frames_empty_channel() {
        let (tx, rx) = mpsc::channel::<FrameData>();
        drop(tx); // Close immediately
        let frames = collect_frames(rx, 30);
        assert!(frames.is_empty());
    }

    #[test]
    fn collect_frames_receives_frames() {
        let (tx, rx) = mpsc::channel::<FrameData>();

        // Send a few frames then close
        for i in 0..3 {
            tx.send(FrameData {
                rgba: vec![0u8; 4 * 10 * 10],
                width: 10,
                height: 10,
                timestamp_secs: i as f64 * 0.1,
            })
            .unwrap();
        }
        drop(tx);

        let frames = collect_frames(rx, 30);
        assert_eq!(frames.len(), 3);
        assert!((frames[2].timestamp_secs - 0.2).abs() < f64::EPSILON);
    }

    #[test]
    fn collect_frames_respects_max_duration() {
        let (tx, rx) = mpsc::channel::<FrameData>();

        // Spawn a thread that sends frames indefinitely
        let sender = std::thread::spawn(move || {
            let mut i = 0;
            loop {
                let result = tx.send(FrameData {
                    rgba: vec![0u8; 4],
                    width: 1,
                    height: 1,
                    timestamp_secs: i as f64 * 0.1,
                });
                if result.is_err() {
                    break;
                }
                i += 1;
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
        });

        // Collect with 1 second max duration
        let frames = collect_frames(rx, 1);
        drop(sender);

        // Should have stopped after ~1 second
        assert!(!frames.is_empty());
    }
}
