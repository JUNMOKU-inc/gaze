use tracing_appender::rolling;
use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter, Layer};

/// Initialize the tracing subscriber with layered outputs:
///
/// - **Console**: human-readable, filtered by `RUST_LOG` (default: `gaze=debug,warn`)
/// - **File**: JSON-structured, daily rotation, kept for 7 days
///
/// Log directory: `~/Library/Logs/Gaze/` (macOS) or platform equivalent.
pub fn init() {
    let log_dir = log_directory();

    // Ensure directory exists
    if let Err(e) = std::fs::create_dir_all(&log_dir) {
        eprintln!("Failed to create log directory {}: {e}", log_dir.display());
    }

    // Layer 1: Console (pretty, for development)
    let console_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("snapforge=debug,warn"));
    let console_layer = fmt::layer()
        .with_target(true)
        .with_span_events(fmt::format::FmtSpan::CLOSE)
        .with_filter(console_filter);

    // Layer 2: File (JSON, for diagnostics & support)
    let file_appender = rolling::Builder::new()
        .rotation(rolling::Rotation::DAILY)
        .max_log_files(7)
        .filename_prefix("gaze")
        .filename_suffix("log")
        .build(&log_dir)
        .expect("Failed to create file appender");

    let file_filter = EnvFilter::new("snapforge=debug,warn");
    let file_layer = fmt::layer()
        .json()
        .with_span_events(fmt::format::FmtSpan::CLOSE)
        .with_writer(file_appender)
        .with_filter(file_filter);

    tracing_subscriber::registry()
        .with(console_layer)
        .with(file_layer)
        .init();

    tracing::info!(log_dir = %log_dir.display(), "Logging initialized");
}

/// Returns the platform-appropriate log directory.
fn log_directory() -> std::path::PathBuf {
    directories::ProjectDirs::from("dev", "gazeapp", "gaze")
        .map(|dirs| dirs.data_local_dir().join("logs"))
        .unwrap_or_else(|| {
            let mut p = dirs_fallback();
            p.push("gaze");
            p.push("logs");
            p
        })
}

fn dirs_fallback() -> std::path::PathBuf {
    #[cfg(target_os = "macos")]
    {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
        std::path::PathBuf::from(home).join("Library/Logs")
    }
    #[cfg(not(target_os = "macos"))]
    {
        std::env::temp_dir()
    }
}
