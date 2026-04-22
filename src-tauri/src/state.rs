use snapforge_capture::CaptureEngine;
use snapforge_core::CaptureMetadata;
use std::sync::{Arc, Mutex, RwLock};

use crate::recording::RecordingHandle;

pub struct AppState {
    pub capture_engine: Arc<dyn CaptureEngine>,
    pub default_provider: RwLock<snapforge_pipeline::LlmProvider>,
    /// Most recent capture, stored so the preview window can fetch it on mount.
    pub latest_capture: RwLock<Option<CaptureMetadata>>,
    /// Active recording session, if any
    pub recording: Mutex<Option<RecordingHandle>>,
    /// Serializes `update_settings` so concurrent invocations can't race on
    /// read-modify-write of settings.json + AppState + OS login item state.
    pub settings_mu: Mutex<()>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            capture_engine: Arc::from(snapforge_capture::create_engine()),
            default_provider: RwLock::new(snapforge_pipeline::LlmProvider::Claude),
            latest_capture: RwLock::new(None),
            recording: Mutex::new(None),
            settings_mu: Mutex::new(()),
        }
    }
}
