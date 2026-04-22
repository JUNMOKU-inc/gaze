# Gaze — Rust Coding Guidelines

> Practical, opinionated guidelines for the Tauri v2 Rust backend.
> Reference implementation: [Cap](https://github.com/CapSoftware/Cap) (Tauri v2 + Rust screen recorder).

---

## Project Structure

```
gaze/
├── Cargo.toml                    # Workspace root
├── crates/
│   ├── snapforge-capture/        # Screen capture abstraction (platform-independent trait)
│   ├── snapforge-pipeline/       # Image processing, LLM optimization, GIF/WebP encoding
│   └── snapforge-ocr/            # OCR abstraction (Vision on macOS, WinOCR on Windows)
├── apps/
│   └── desktop/
│       └── src-tauri/
│           ├── Cargo.toml        # Tauri app — depends on workspace crates
│           └── src/
│               ├── main.rs       # Entry point (thin — calls lib::run)
│               ├── lib.rs        # App setup, plugin registration, command routing
│               ├── commands/     # Tauri commands grouped by domain
│               │   ├── capture.rs
│               │   ├── pipeline.rs
│               │   ├── clipboard.rs
│               │   └── settings.rs
│               ├── platform/     # Platform-specific glue code
│               │   ├── mod.rs
│               │   ├── macos.rs
│               │   └── windows.rs
│               ├── state.rs      # Tauri managed state definitions
│               └── error.rs      # Unified error types for commands
```

**Rule: Extract reusable logic into workspace crates.** The `src-tauri` app should only contain Tauri-specific wiring (commands, state, window management). Business logic lives in `crates/`.

Cap follows this pattern — `cap-recording`, `cap-editor`, `cap-rendering` are standalone crates consumed by the Tauri app.

---

## Naming Conventions

| Item | Convention | Example |
|------|-----------|---------|
| Crates | `snapforge-{domain}` | `snapforge-capture` |
| Modules | `snake_case` | `capture_engine.rs` |
| Types/Traits | `PascalCase` | `CaptureEngine`, `PipelineConfig` |
| Functions | `snake_case` | `capture_area()` |
| Tauri commands | `snake_case` (maps to camelCase on TS side) | `#[tauri::command] fn capture_area()` |
| Constants | `SCREAMING_SNAKE_CASE` | `MAX_CAPTURE_WIDTH` |
| Feature flags | `kebab-case` | `feature = "ocr-support"` |
| Error variants | `PascalCase`, noun phrase | `CaptureError::PermissionDenied` |

---

## Error Handling

Use `thiserror` for library crates, `anyhow` sparingly in the app layer. Tauri commands return `Result<T, String>` or a serializable error.

```rust
// crates/snapforge-capture/src/error.rs
use thiserror::Error;

#[derive(Debug, Error)]
pub enum CaptureError {
    #[error("Screen recording permission not granted")]
    PermissionDenied,
    #[error("Display {0} not found")]
    DisplayNotFound(u32),
    #[error("Capture failed: {0}")]
    Internal(#[from] anyhow::Error),
}
```

```rust
// src-tauri/src/error.rs — Tauri-facing wrapper
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct CommandError {
    pub message: String,
    pub code: String,
}

impl<E: std::error::Error> From<E> for CommandError {
    fn from(err: E) -> Self {
        CommandError {
            message: err.to_string(),
            code: std::any::type_name::<E>().to_string(),
        }
    }
}

// This makes `Result<T, CommandError>` work in #[tauri::command]
impl From<CommandError> for tauri::ipc::InvokeError {
    fn from(err: CommandError) -> Self {
        tauri::ipc::InvokeError::from(serde_json::to_value(err).unwrap())
    }
}
```

**Rules:**
- Never `.unwrap()` in production code. Use `.expect("reason")` only for truly impossible states.
- Propagate errors with `?`. Convert at the boundary (Tauri command layer).
- Log errors with `tracing::error!` before returning to the frontend.

---

## Async & Concurrency

Tauri v2 runs on Tokio. Follow these patterns:

```rust
// GOOD: Async command — Tauri spawns this on the Tokio runtime
#[tauri::command]
async fn capture_area(x: i32, y: i32, w: u32, h: u32) -> Result<Vec<u8>, CommandError> {
    let image = snapforge_capture::capture_region(x, y, w, h).await?;
    Ok(image)
}

// GOOD: CPU-heavy work — offload to blocking thread pool
#[tauri::command]
async fn optimize_for_llm(image_data: Vec<u8>, provider: String) -> Result<Vec<u8>, CommandError> {
    let result = tokio::task::spawn_blocking(move || {
        snapforge_pipeline::optimize(&image_data, &provider)
    }).await.map_err(|e| anyhow::anyhow!("Task join error: {e}"))??;
    Ok(result)
}
```

**Rules:**
- Use `async fn` for I/O-bound Tauri commands (file ops, network).
- Use `tokio::task::spawn_blocking` for CPU-bound work (image encoding, OCR).
- Never block the Tokio runtime with synchronous loops or `std::thread::sleep`.
- Share state via `Arc<Mutex<T>>` or `Arc<RwLock<T>>` registered as Tauri managed state.

---

## Platform Abstraction Pattern

Use `#[cfg(target_os)]` at the module level and re-export a unified API. This is exactly how Cap structures `crates/camera/`.

```rust
// crates/snapforge-capture/src/lib.rs
#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "windows")]
mod windows;

#[cfg(target_os = "macos")]
use macos as platform;
#[cfg(target_os = "windows")]
use windows as platform;

pub use platform::capture_screen;
pub use platform::capture_region;
pub use platform::list_displays;
```

```rust
// crates/snapforge-capture/src/macos.rs
use scap::{capturer::Capturer, frame::Frame};

pub fn capture_screen(display_id: u32) -> Result<Vec<u8>, CaptureError> {
    // ScreenCaptureKit implementation
}

pub fn capture_region(x: i32, y: i32, w: u32, h: u32) -> Result<Vec<u8>, CaptureError> {
    // ...
}

pub fn list_displays() -> Vec<DisplayInfo> {
    // ...
}
```

For the Tauri app layer, use `#[cfg]` inline for small platform-specific branches (see Cap's `platform/mod.rs`):

```rust
#[tauri::command]
pub fn open_permission_settings(permission: OSPermission) {
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg("x-apple.systempreferences:com.apple.preference.security?Privacy_ScreenCapture")
            .spawn()
            .ok();
    }
    #[cfg(target_os = "windows")]
    {
        // Windows-specific implementation
    }
}
```

**Rule:** Keep `#[cfg]` blocks as small as possible. Prefer module-level separation in crates; inline `#[cfg]` only for trivial differences.

---

## Tauri Command Pattern

```rust
// src-tauri/src/commands/capture.rs
use tauri::State;
use crate::state::AppState;
use crate::error::CommandError;

/// Capture a screen region and return optimized image bytes.
/// Called from TS: `invoke('capture_region', { x, y, width, height })`
#[tauri::command]
pub async fn capture_region(
    state: State<'_, AppState>,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
) -> Result<Vec<u8>, CommandError> {
    tracing::info!(x, y, width, height, "Capturing region");

    let image = snapforge_capture::capture_region(x, y, width, height)
        .await
        .map_err(|e| {
            tracing::error!("Capture failed: {e}");
            e
        })?;

    let settings = state.settings.read().await;
    let optimized = tokio::task::spawn_blocking(move || {
        snapforge_pipeline::optimize(&image, &settings.default_provider)
    }).await??;

    Ok(optimized)
}
```

```rust
// src-tauri/src/lib.rs — register commands
pub fn run() {
    tauri::Builder::default()
        .manage(AppState::default())
        .invoke_handler(tauri::generate_handler![
            commands::capture::capture_region,
            commands::capture::capture_window,
            commands::capture::list_displays,
            commands::pipeline::optimize_for_llm,
            commands::clipboard::copy_to_clipboard,
            commands::settings::get_settings,
            commands::settings::update_settings,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

**Rules:**
- One file per command domain. Never put all commands in `lib.rs`.
- Accept `State<'_, T>` for shared data — never use global statics.
- Use `tracing` (not `println!`) for all logging.
- Commands are the only public API surface to the frontend. Keep them thin — delegate to crate logic.

---

## Testing Strategy

### Unit Tests (in crates)

Place tests in `#[cfg(test)] mod tests` at the bottom of each file. Test pure logic without Tauri dependency.

```rust
// crates/snapforge-pipeline/src/optimizer.rs
pub fn calculate_optimal_dimensions(
    width: u32, height: u32, max_tokens: u32, provider: &str,
) -> (u32, u32) {
    match provider {
        "claude" => {
            let scale = (max_tokens as f64 * 750.0 / (width as f64 * height as f64)).sqrt();
            let new_w = (width as f64 * scale).min(1568.0) as u32;
            let new_h = (height as f64 * scale).min(1568.0) as u32;
            (new_w, new_h)
        }
        _ => (width.min(2048), height.min(2048)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn claude_optimal_dimensions_respects_max() {
        let (w, h) = calculate_optimal_dimensions(3840, 2160, 1600, "claude");
        assert!(w <= 1568);
        assert!(h <= 1568);
    }

    #[test]
    fn unknown_provider_caps_at_2048() {
        let (w, h) = calculate_optimal_dimensions(4000, 3000, 1600, "unknown");
        assert_eq!(w, 2048);
        assert_eq!(h, 2048);
    }
}
```

### Trait-Based Mocking

Define traits in crates for any component that touches the OS. Implement a mock for tests.

```rust
// crates/snapforge-capture/src/lib.rs
pub trait CaptureEngine: Send + Sync {
    fn capture_region(&self, x: i32, y: i32, w: u32, h: u32) -> Result<Vec<u8>, CaptureError>;
    fn list_displays(&self) -> Result<Vec<DisplayInfo>, CaptureError>;
}

// Production implementation
pub struct NativeCaptureEngine;
impl CaptureEngine for NativeCaptureEngine {
    fn capture_region(&self, x: i32, y: i32, w: u32, h: u32) -> Result<Vec<u8>, CaptureError> {
        platform::capture_region(x, y, w, h)
    }
    fn list_displays(&self) -> Result<Vec<DisplayInfo>, CaptureError> {
        Ok(platform::list_displays())
    }
}

// Test mock
#[cfg(test)]
pub struct MockCaptureEngine {
    pub fake_image: Vec<u8>,
}

#[cfg(test)]
impl CaptureEngine for MockCaptureEngine {
    fn capture_region(&self, _x: i32, _y: i32, _w: u32, _h: u32) -> Result<Vec<u8>, CaptureError> {
        Ok(self.fake_image.clone())
    }
    fn list_displays(&self) -> Result<Vec<DisplayInfo>, CaptureError> {
        Ok(vec![DisplayInfo { id: 1, name: "Mock Display".into(), width: 1920, height: 1080 }])
    }
}
```

Then in the pipeline crate, accept `impl CaptureEngine` or `&dyn CaptureEngine` so tests can inject mocks.

### Integration Tests

Place in `tests/` directory at crate root. Test cross-module workflows.

```rust
// crates/snapforge-pipeline/tests/full_pipeline.rs
use snapforge_pipeline::{optimize, PipelineConfig};

#[test]
fn png_input_produces_valid_webp_output() {
    let png_bytes = include_bytes!("fixtures/test_screenshot.png");
    let config = PipelineConfig { provider: "claude".into(), max_tokens: 1600 };
    let result = optimize(png_bytes, &config).unwrap();

    // Verify WebP magic bytes
    assert_eq!(&result[0..4], b"RIFF");
    assert_eq!(&result[8..12], b"WEBP");
}
```

### Testing Tauri Commands

Test command logic indirectly — keep commands thin, test the underlying crate functions directly. For full integration, use `tauri::test`:

```rust
#[cfg(test)]
mod tests {
    use tauri::test::{mock_builder, MockRuntime};

    #[test]
    fn test_app_starts() {
        let app = mock_builder()
            .invoke_handler(tauri::generate_handler![super::commands::capture::list_displays])
            .build(tauri::generate_context!())
            .unwrap();
        // App built successfully with commands registered
    }
}
```

---

## Dependencies Policy

| Category | Allowed | Rationale |
|----------|---------|-----------|
| Error handling | `thiserror`, `anyhow` | `thiserror` for libs, `anyhow` for app |
| Serialization | `serde`, `serde_json` | Required by Tauri IPC |
| Async | `tokio` (workspace version) | Tauri's runtime; pin to workspace version |
| Logging | `tracing`, `tracing-subscriber` | Structured logging; Cap uses this too |
| Image | `image`, `webp`, `gifski` | Core to pipeline |
| Capture | `scap` | Cap team's cross-platform crate |
| CLI | `clap` with derive | For future CLI/MCP features |

**Rules:**
- Pin workspace dependencies in root `Cargo.toml` under `[workspace.dependencies]`. Reference with `dep.workspace = true` in member crates.
- Audit new dependencies: check download count, last update, license (MIT/Apache-2.0 only).
- No `unsafe` without a comment explaining why and a `// SAFETY:` annotation.
- Prefer `#[cfg(feature = "...")]` to make heavy optional deps (gifski, OCR) opt-in.

---

## Code Review Checklist

Before merging any Rust PR, verify:

- [ ] `cargo clippy -- -D warnings` passes with zero warnings
- [ ] `cargo test --workspace` passes
- [ ] No `.unwrap()` in non-test code (search for it)
- [ ] Tauri commands are thin wrappers — logic lives in workspace crates
- [ ] New platform-specific code uses `#[cfg(target_os)]` at module level when possible
- [ ] Errors are typed (`thiserror`) in crates, not raw `String`
- [ ] CPU-bound work uses `spawn_blocking`, not blocking the async runtime
- [ ] `tracing` used for logging (not `println!` / `eprintln!`)
- [ ] New dependencies added to `[workspace.dependencies]`, not inline in member crates
- [ ] Public functions have doc comments (`///`) explaining purpose and error conditions
- [ ] Test coverage exists for new pure functions; mocks used for OS-dependent logic
