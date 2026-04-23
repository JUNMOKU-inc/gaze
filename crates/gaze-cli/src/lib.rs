use clap::{Args, Parser, Subcommand, ValueEnum};
use serde::Serialize;
use snapforge_core::{
    apply_annotations_to_processed_capture, build_llm_prompt_hint_for_language,
    load_settings_or_default, process_image_bytes, process_image_bytes_with_mode,
    process_rgba_capture_with_mode, Annotation, CaptureProcessingMode, ProcessedCapture, Settings,
    SETTINGS_KEYS,
};
use std::ffi::OsString;
use std::io::Write;
use std::path::{Path, PathBuf};

pub const EXIT_SUCCESS: u8 = 0;
pub const EXIT_FAILURE: u8 = 1;
pub const EXIT_USAGE: u8 = 2;
pub const EXIT_CANCELLED: u8 = 3;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapturedImage {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DisplayRecord {
    pub id: u32,
    pub name: String,
    pub width: u32,
    pub height: u32,
    pub scale_factor: f64,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct WindowRecord {
    pub id: u32,
    pub title: String,
    pub app_name: String,
    pub is_on_screen: bool,
}

pub trait CaptureBackend {
    fn has_permission(&self) -> bool;
    fn list_displays(&self) -> Result<Vec<DisplayRecord>, String>;
    fn list_windows(&self) -> Result<Vec<WindowRecord>, String>;
    fn capture_fullscreen(&self, display_id: Option<u32>) -> Result<CapturedImage, String>;
    fn capture_window(&self, window_id: u32) -> Result<CapturedImage, String>;
    fn capture_area_interactive(&self) -> Result<Option<Vec<u8>>, String>;
    fn capture_window_interactive(&self) -> Result<Option<Vec<u8>>, String>;
    fn copy_rgba_to_clipboard(
        &self,
        rgba_data: &[u8],
        width: u32,
        height: u32,
    ) -> Result<(), String>;
}

#[derive(Debug, Parser)]
#[command(name = "gaze", version, about = "LLM-optimized screen capture CLI")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    Capture(CaptureArgs),
    List(ListArgs),
    Optimize(OptimizeArgs),
    /// View and modify persisted settings shared with the Gaze desktop app
    Settings(SettingsArgs),
    Version,
}

#[derive(Debug, Args)]
struct SettingsArgs {
    /// Override the settings file path (primarily for tests and scripts).
    #[arg(long, global = true)]
    config: Option<PathBuf>,

    #[command(subcommand)]
    action: SettingsCommand,
}

#[derive(Debug, Subcommand)]
enum SettingsCommand {
    /// Print all settings or a single field as JSON
    Get {
        /// Key to read (e.g. `language`, `maxDimension.pixels`). Omit to print all.
        key: Option<String>,
    },
    /// Update a single setting field
    Set {
        /// Key to write (e.g. `language`, `launchAtLogin`).
        key: String,
        /// New value. Parsed as JSON when possible so `true`, `42`, `"foo"` all work; bare
        /// identifiers like `ja` are treated as strings.
        value: String,
    },
    /// Reset all settings to defaults
    Reset,
    /// Print the resolved settings file path
    Path,
    /// List every settable key
    Keys,
}

#[derive(Debug, Args)]
struct CaptureArgs {
    #[arg(short, long, value_enum, default_value_t = CaptureMode::Full)]
    mode: CaptureMode,

    #[arg(short, long)]
    display: Option<u32>,

    #[arg(short, long)]
    window: Option<u32>,

    #[arg(short, long)]
    output: Option<PathBuf>,

    #[arg(long)]
    copy: bool,

    #[arg(long)]
    raw: bool,

    #[arg(short, long, value_enum, default_value_t = OutputFormat::Json)]
    format: OutputFormat,

    #[command(flatten)]
    annotations: AnnotationArgs,
}

#[derive(Debug, Args)]
struct OptimizeArgs {
    input: PathBuf,

    #[arg(short, long)]
    output: Option<PathBuf>,

    #[arg(long)]
    copy: bool,

    #[arg(short, long, value_enum, default_value_t = OutputFormat::Json)]
    format: OutputFormat,

    #[command(flatten)]
    annotations: AnnotationArgs,
}

#[derive(Debug, Args, Default, Clone, PartialEq, Eq)]
struct AnnotationArgs {
    /// Add a numbered pin in absolute pixels: `--pin 640,320[:note]`
    #[arg(long = "pin", value_name = "X,Y[:NOTE]")]
    pins: Vec<String>,

    /// Add a labelled rectangle in absolute pixels: `--rect 120,80,400,200[:note]`
    #[arg(long = "rect", value_name = "X,Y,W,H[:NOTE]")]
    rects: Vec<String>,
}

#[derive(Debug, Args)]
struct ListArgs {
    #[command(subcommand)]
    target: ListTarget,
}

#[derive(Debug, Subcommand)]
enum ListTarget {
    Displays,
    Windows,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum CaptureMode {
    Full,
    Area,
    Window,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum OutputFormat {
    Json,
    Base64,
    Path,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
struct CaptureCommandOutput {
    original_width: u32,
    original_height: u32,
    optimized_width: u32,
    optimized_height: u32,
    file_size: usize,
    timestamp: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    image_base64: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    output_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    prompt_hint: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    annotations: Option<Vec<Annotation>>,
}

impl CaptureCommandOutput {
    fn from_processed(
        processed: &ProcessedCapture,
        output_path: Option<&Path>,
        prompt_hint: Option<String>,
        annotations: &[Annotation],
    ) -> Self {
        let metadata = &processed.metadata;
        Self {
            original_width: metadata.original_width,
            original_height: metadata.original_height,
            optimized_width: metadata.optimized_width,
            optimized_height: metadata.optimized_height,
            file_size: metadata.file_size,
            timestamp: metadata.timestamp.clone(),
            image_base64: output_path.is_none().then(|| metadata.image_base64.clone()),
            output_path: output_path.map(|path| path.display().to_string()),
            prompt_hint,
            annotations: (!annotations.is_empty()).then(|| annotations.to_vec()),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ErrorKind {
    Usage,
    Runtime,
    Cancelled,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RunError {
    kind: ErrorKind,
    message: String,
}

impl RunError {
    fn usage(message: impl Into<String>) -> Self {
        Self {
            kind: ErrorKind::Usage,
            message: message.into(),
        }
    }

    fn runtime(message: impl Into<String>) -> Self {
        Self {
            kind: ErrorKind::Runtime,
            message: message.into(),
        }
    }

    fn cancelled(message: impl Into<String>) -> Self {
        Self {
            kind: ErrorKind::Cancelled,
            message: message.into(),
        }
    }
}

enum CaptureTarget {
    Full { display_id: Option<u32> },
    AreaInteractive,
    WindowDirect { window_id: u32 },
    WindowInteractive,
}

pub fn run_cli<I, B, W, E>(args: I, backend: &B, stdout: &mut W, stderr: &mut E) -> u8
where
    I: IntoIterator,
    I::Item: Into<OsString> + Clone,
    B: CaptureBackend,
    W: Write,
    E: Write,
{
    let cli = match Cli::try_parse_from(args) {
        Ok(cli) => cli,
        Err(err) => {
            let exit_code = match err.kind() {
                clap::error::ErrorKind::DisplayHelp | clap::error::ErrorKind::DisplayVersion => {
                    EXIT_SUCCESS
                }
                _ => EXIT_USAGE,
            };
            let writer = if exit_code == EXIT_SUCCESS {
                stdout as &mut dyn Write
            } else {
                stderr as &mut dyn Write
            };
            let _ = write!(writer, "{err}");
            return exit_code;
        }
    };

    match execute_command(cli.command, backend, stdout) {
        Ok(()) => EXIT_SUCCESS,
        Err(err) => {
            let _ = writeln!(stderr, "{}", err.message);
            match err.kind {
                ErrorKind::Usage => EXIT_USAGE,
                ErrorKind::Runtime => EXIT_FAILURE,
                ErrorKind::Cancelled => EXIT_CANCELLED,
            }
        }
    }
}

fn execute_command<B, W>(command: Commands, backend: &B, stdout: &mut W) -> Result<(), RunError>
where
    B: CaptureBackend,
    W: Write,
{
    match command {
        Commands::Capture(args) => run_capture(args, backend, stdout),
        Commands::List(args) => run_list(args, backend, stdout),
        Commands::Optimize(args) => run_optimize(args, backend, stdout),
        Commands::Settings(args) => run_settings(args, stdout),
        Commands::Version => writeln!(stdout, "gaze {}", env!("CARGO_PKG_VERSION"))
            .map_err(|e| RunError::runtime(format!("Failed to write version output: {e}"))),
    }
}

fn resolve_settings_path(override_path: Option<PathBuf>) -> Result<PathBuf, RunError> {
    if let Some(path) = override_path {
        return Ok(path);
    }
    snapforge_core::default_settings_path().map_err(|e| {
        RunError::runtime(format!(
            "Could not resolve default settings path: {e}. Pass --config <PATH> to override."
        ))
    })
}

fn run_settings<W: Write>(args: SettingsArgs, stdout: &mut W) -> Result<(), RunError> {
    let path = resolve_settings_path(args.config)?;

    match args.action {
        SettingsCommand::Path => writeln!(stdout, "{}", path.display())
            .map_err(|e| RunError::runtime(format!("Failed to write settings path: {e}"))),
        SettingsCommand::Keys => {
            for key in SETTINGS_KEYS {
                writeln!(stdout, "{key}")
                    .map_err(|e| RunError::runtime(format!("Failed to write key list: {e}")))?;
            }
            Ok(())
        }
        SettingsCommand::Get { key } => {
            let settings = load_settings_for_cli(&path)?;
            match key {
                None => emit_json(stdout, &settings),
                Some(key) => {
                    let value = snapforge_core::get_setting_field(&settings, &key)
                        .map_err(map_settings_error)?;
                    emit_json(stdout, &value)
                }
            }
        }
        SettingsCommand::Set { key, value } => {
            let mut settings = load_settings_for_cli(&path)?;
            snapforge_core::set_setting_field(&mut settings, &key, &value)
                .map_err(map_settings_error)?;
            snapforge_core::save_settings(&path, &settings).map_err(map_settings_error)?;
            writeln!(stdout, "{key} updated")
                .map_err(|e| RunError::runtime(format!("Failed to write confirmation: {e}")))
        }
        SettingsCommand::Reset => {
            let defaults = Settings::default();
            snapforge_core::save_settings(&path, &defaults).map_err(map_settings_error)?;
            writeln!(stdout, "settings reset to defaults")
                .map_err(|e| RunError::runtime(format!("Failed to write confirmation: {e}")))
        }
    }
}

fn load_settings_for_cli(path: &Path) -> Result<Settings, RunError> {
    snapforge_core::load_settings(path).map_err(map_settings_error)
}

fn resolve_prompt_language_for_cli() -> String {
    match snapforge_core::default_settings_path() {
        Ok(path) => {
            let settings = load_settings_or_default(&path);
            let language = settings.language.trim();
            if language.is_empty() {
                "en".to_string()
            } else {
                language.to_string()
            }
        }
        Err(_) => "en".to_string(),
    }
}

fn map_settings_error(err: snapforge_core::SettingsError) -> RunError {
    use snapforge_core::SettingsError;
    match err {
        SettingsError::UnknownKey(_) | SettingsError::InvalidValue { .. } => {
            RunError::usage(err.to_string())
        }
        _ => RunError::runtime(err.to_string()),
    }
}

fn run_capture<B, W>(args: CaptureArgs, backend: &B, stdout: &mut W) -> Result<(), RunError>
where
    B: CaptureBackend,
    W: Write,
{
    ensure_permission(backend)?;
    validate_output_requirements(args.format, args.output.as_deref())?;

    let processing_mode = if args.raw {
        CaptureProcessingMode::Raw
    } else {
        CaptureProcessingMode::Optimized
    };

    let target = resolve_capture_target(&args)?;
    let mut processed = match target {
        CaptureTarget::Full { display_id } => {
            let capture = backend
                .capture_fullscreen(display_id)
                .map_err(RunError::runtime)?;
            process_rgba_capture_with_mode(
                &capture.rgba,
                capture.width,
                capture.height,
                processing_mode,
            )
            .map_err(|e| RunError::runtime(e.to_string()))?
        }
        CaptureTarget::AreaInteractive => {
            let bytes = backend
                .capture_area_interactive()
                .map_err(RunError::runtime)?
                .ok_or_else(|| RunError::cancelled("Capture cancelled by user"))?;
            process_image_bytes_with_mode(&bytes, processing_mode)
                .map_err(|e| RunError::runtime(e.to_string()))?
        }
        CaptureTarget::WindowDirect { window_id } => {
            let capture = backend
                .capture_window(window_id)
                .map_err(RunError::runtime)?;
            process_rgba_capture_with_mode(
                &capture.rgba,
                capture.width,
                capture.height,
                processing_mode,
            )
            .map_err(|e| RunError::runtime(e.to_string()))?
        }
        CaptureTarget::WindowInteractive => {
            let bytes = backend
                .capture_window_interactive()
                .map_err(RunError::runtime)?
                .ok_or_else(|| RunError::cancelled("Capture cancelled by user"))?;
            process_image_bytes_with_mode(&bytes, processing_mode)
                .map_err(|e| RunError::runtime(e.to_string()))?
        }
    };

    let annotations = parse_annotations(
        &args.annotations,
        processed.metadata.optimized_width,
        processed.metadata.optimized_height,
    )?;
    let prompt_language = resolve_prompt_language_for_cli();
    let prompt_hint = build_llm_prompt_hint_for_language(&annotations, &prompt_language);
    if !annotations.is_empty() {
        processed = apply_annotations_to_processed_capture(processed, &annotations)
            .map_err(|e| RunError::runtime(e.to_string()))?;
    }

    write_output_if_requested(args.output.as_deref(), &processed.encoded)?;

    if args.copy {
        backend
            .copy_rgba_to_clipboard(
                &processed.rgba,
                processed.metadata.optimized_width,
                processed.metadata.optimized_height,
            )
            .map_err(RunError::runtime)?;
    }

    emit_capture_output(
        stdout,
        args.format,
        &processed,
        args.output.as_deref(),
        prompt_hint,
        &annotations,
    )
}

fn run_list<B, W>(args: ListArgs, backend: &B, stdout: &mut W) -> Result<(), RunError>
where
    B: CaptureBackend,
    W: Write,
{
    ensure_permission(backend)?;

    match args.target {
        ListTarget::Displays => {
            emit_json(stdout, &backend.list_displays().map_err(RunError::runtime)?)
        }
        ListTarget::Windows => {
            emit_json(stdout, &backend.list_windows().map_err(RunError::runtime)?)
        }
    }
}

fn run_optimize<B, W>(args: OptimizeArgs, backend: &B, stdout: &mut W) -> Result<(), RunError>
where
    B: CaptureBackend,
    W: Write,
{
    validate_output_requirements(args.format, args.output.as_deref())?;

    let input_bytes = std::fs::read(&args.input)
        .map_err(|e| RunError::runtime(format!("Failed to read input file: {e}")))?;
    let mut processed =
        process_image_bytes(&input_bytes).map_err(|e| RunError::runtime(e.to_string()))?;

    let annotations = parse_annotations(
        &args.annotations,
        processed.metadata.optimized_width,
        processed.metadata.optimized_height,
    )?;
    let prompt_language = resolve_prompt_language_for_cli();
    let prompt_hint = build_llm_prompt_hint_for_language(&annotations, &prompt_language);
    if !annotations.is_empty() {
        processed = apply_annotations_to_processed_capture(processed, &annotations)
            .map_err(|e| RunError::runtime(e.to_string()))?;
    }

    write_output_if_requested(args.output.as_deref(), &processed.encoded)?;

    if args.copy {
        backend
            .copy_rgba_to_clipboard(
                &processed.rgba,
                processed.metadata.optimized_width,
                processed.metadata.optimized_height,
            )
            .map_err(RunError::runtime)?;
    }

    emit_capture_output(
        stdout,
        args.format,
        &processed,
        args.output.as_deref(),
        prompt_hint,
        &annotations,
    )
}

fn ensure_permission<B: CaptureBackend>(backend: &B) -> Result<(), RunError> {
    if backend.has_permission() {
        Ok(())
    } else {
        Err(RunError::runtime(
            "Screen recording permission not granted. Enable it in System Settings and retry.",
        ))
    }
}

fn validate_output_requirements(
    format: OutputFormat,
    output_path: Option<&Path>,
) -> Result<(), RunError> {
    if format == OutputFormat::Path && output_path.is_none() {
        return Err(RunError::usage(
            "--format path requires --output <PATH> to be set",
        ));
    }

    Ok(())
}

fn parse_annotations(
    args: &AnnotationArgs,
    image_width: u32,
    image_height: u32,
) -> Result<Vec<Annotation>, RunError> {
    let mut annotations = Vec::new();

    for (index, spec) in args.pins.iter().enumerate() {
        let (coords, note) = split_annotation_spec(spec);
        let values = parse_f32_list(coords, 2, "--pin")?;
        annotations.push(Annotation::pin(
            index,
            normalize_coordinate(values[0], image_width, "--pin x")?,
            normalize_coordinate(values[1], image_height, "--pin y")?,
            note,
        ));
    }

    for (index, spec) in args.rects.iter().enumerate() {
        let (coords, note) = split_annotation_spec(spec);
        let values = parse_f32_list(coords, 4, "--rect")?;
        annotations.push(Annotation::rectangle(
            index,
            normalize_coordinate(values[0], image_width, "--rect x")?,
            normalize_coordinate(values[1], image_height, "--rect y")?,
            normalize_extent(values[2], image_width, "--rect width")?,
            normalize_extent(values[3], image_height, "--rect height")?,
            note,
        ));
    }

    Ok(annotations)
}

fn split_annotation_spec(spec: &str) -> (&str, Option<String>) {
    if let Some((coords, note)) = spec.split_once(':') {
        (coords, Some(note.trim().to_string()))
    } else {
        (spec, None)
    }
}

fn parse_f32_list(spec: &str, expected: usize, flag: &str) -> Result<Vec<f32>, RunError> {
    let values: Result<Vec<f32>, _> = spec
        .split(',')
        .map(|part| part.trim().parse::<f32>())
        .collect();
    let values =
        values.map_err(|e| RunError::usage(format!("{flag} expects numeric values: {e}")))?;
    if values.len() != expected {
        return Err(RunError::usage(format!(
            "{flag} expects {expected} comma-separated numbers"
        )));
    }
    Ok(values)
}

fn normalize_coordinate(value: f32, max: u32, field: &str) -> Result<f32, RunError> {
    if value < 0.0 {
        return Err(RunError::usage(format!("{field} must be >= 0")));
    }
    if max == 0 {
        return Err(RunError::runtime(
            "Cannot normalize annotations for a zero-sized image".to_string(),
        ));
    }
    Ok((value / max as f32).clamp(0.0, 1.0))
}

fn normalize_extent(value: f32, max: u32, field: &str) -> Result<f32, RunError> {
    if value <= 0.0 {
        return Err(RunError::usage(format!("{field} must be > 0")));
    }
    normalize_coordinate(value, max, field)
}

fn resolve_capture_target(args: &CaptureArgs) -> Result<CaptureTarget, RunError> {
    if args.display.is_some()
        && (args.mode == CaptureMode::Area || args.mode == CaptureMode::Window)
    {
        return Err(RunError::usage(
            "--display can only be used with full capture mode",
        ));
    }

    if args.display.is_some() && args.window.is_some() {
        return Err(RunError::usage(
            "--display cannot be combined with --window",
        ));
    }

    match args.mode {
        CaptureMode::Area => {
            if args.window.is_some() {
                return Err(RunError::usage(
                    "--window cannot be combined with area capture mode",
                ));
            }
            Ok(CaptureTarget::AreaInteractive)
        }
        CaptureMode::Window => {
            if let Some(window_id) = args.window {
                Ok(CaptureTarget::WindowDirect { window_id })
            } else {
                Ok(CaptureTarget::WindowInteractive)
            }
        }
        CaptureMode::Full => {
            if let Some(window_id) = args.window {
                Ok(CaptureTarget::WindowDirect { window_id })
            } else {
                Ok(CaptureTarget::Full {
                    display_id: args.display,
                })
            }
        }
    }
}

fn write_output_if_requested(output_path: Option<&Path>, encoded: &[u8]) -> Result<(), RunError> {
    if let Some(path) = output_path {
        std::fs::write(path, encoded)
            .map_err(|e| RunError::runtime(format!("Failed to write output file: {e}")))?;
    }

    Ok(())
}

fn emit_capture_output<W: Write>(
    stdout: &mut W,
    format: OutputFormat,
    processed: &ProcessedCapture,
    output_path: Option<&Path>,
    prompt_hint: Option<String>,
    annotations: &[Annotation],
) -> Result<(), RunError> {
    match format {
        OutputFormat::Json => emit_json(
            stdout,
            &CaptureCommandOutput::from_processed(processed, output_path, prompt_hint, annotations),
        ),
        OutputFormat::Base64 => writeln!(stdout, "{}", processed.metadata.image_base64)
            .map_err(|e| RunError::runtime(format!("Failed to write base64 output: {e}"))),
        OutputFormat::Path => {
            let output_path = output_path
                .ok_or_else(|| RunError::usage("--format path requires --output <PATH>"))?;
            writeln!(stdout, "{}", output_path.display())
                .map_err(|e| RunError::runtime(format!("Failed to write path output: {e}")))
        }
    }
}

fn emit_json<W: Write, T: Serialize>(stdout: &mut W, value: &T) -> Result<(), RunError> {
    serde_json::to_writer(&mut *stdout, value)
        .map_err(|e| RunError::runtime(format!("Failed to serialize JSON: {e}")))?;
    writeln!(stdout).map_err(|e| RunError::runtime(format!("Failed to write JSON output: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{DynamicImage, ImageBuffer, Rgba};
    use std::cell::RefCell;
    use tempfile::tempdir;

    struct FakeBackend {
        permission: bool,
        displays: Result<Vec<DisplayRecord>, String>,
        windows: Result<Vec<WindowRecord>, String>,
        fullscreen: Result<CapturedImage, String>,
        window_capture: Result<CapturedImage, String>,
        area_capture: Result<Option<Vec<u8>>, String>,
        interactive_window_capture: Result<Option<Vec<u8>>, String>,
        copy_result: Result<(), String>,
        calls: RefCell<Vec<String>>,
    }

    impl FakeBackend {
        fn successful() -> Self {
            Self {
                permission: true,
                displays: Ok(vec![DisplayRecord {
                    id: 1,
                    name: "Main Display".into(),
                    width: 1920,
                    height: 1080,
                    scale_factor: 2.0,
                }]),
                windows: Ok(vec![WindowRecord {
                    id: 42,
                    title: "Terminal".into(),
                    app_name: "Ghostty".into(),
                    is_on_screen: true,
                }]),
                fullscreen: Ok(sample_capture_image(160, 90)),
                window_capture: Ok(sample_capture_image(120, 80)),
                area_capture: Ok(Some(sample_png(100, 50))),
                interactive_window_capture: Ok(Some(sample_png(140, 60))),
                copy_result: Ok(()),
                calls: RefCell::new(Vec::new()),
            }
        }
    }

    impl CaptureBackend for FakeBackend {
        fn has_permission(&self) -> bool {
            self.permission
        }

        fn list_displays(&self) -> Result<Vec<DisplayRecord>, String> {
            self.calls.borrow_mut().push("list_displays".into());
            self.displays.clone()
        }

        fn list_windows(&self) -> Result<Vec<WindowRecord>, String> {
            self.calls.borrow_mut().push("list_windows".into());
            self.windows.clone()
        }

        fn capture_fullscreen(&self, display_id: Option<u32>) -> Result<CapturedImage, String> {
            self.calls
                .borrow_mut()
                .push(format!("capture_fullscreen:{display_id:?}"));
            self.fullscreen.clone()
        }

        fn capture_window(&self, window_id: u32) -> Result<CapturedImage, String> {
            self.calls
                .borrow_mut()
                .push(format!("capture_window:{window_id}"));
            self.window_capture.clone()
        }

        fn capture_area_interactive(&self) -> Result<Option<Vec<u8>>, String> {
            self.calls
                .borrow_mut()
                .push("capture_area_interactive".into());
            self.area_capture.clone()
        }

        fn capture_window_interactive(&self) -> Result<Option<Vec<u8>>, String> {
            self.calls
                .borrow_mut()
                .push("capture_window_interactive".into());
            self.interactive_window_capture.clone()
        }

        fn copy_rgba_to_clipboard(
            &self,
            _rgba_data: &[u8],
            width: u32,
            height: u32,
        ) -> Result<(), String> {
            self.calls
                .borrow_mut()
                .push(format!("copy:{width}x{height}"));
            self.copy_result.clone()
        }
    }

    fn sample_capture_image(width: u32, height: u32) -> CapturedImage {
        CapturedImage {
            width,
            height,
            rgba: vec![200u8; (width * height * 4) as usize],
        }
    }

    fn sample_png(width: u32, height: u32) -> Vec<u8> {
        let img = ImageBuffer::from_fn(width, height, |_, _| Rgba([255u8, 0, 0, 255]));
        let mut cursor = std::io::Cursor::new(Vec::new());
        DynamicImage::ImageRgba8(img)
            .write_to(&mut cursor, image::ImageFormat::Png)
            .unwrap();
        cursor.into_inner()
    }

    fn run(args: &[&str], backend: &FakeBackend) -> (u8, String, String) {
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let exit_code = run_cli(args.iter().copied(), backend, &mut stdout, &mut stderr);
        (
            exit_code,
            String::from_utf8(stdout).unwrap(),
            String::from_utf8(stderr).unwrap(),
        )
    }

    #[test]
    fn help_flag_returns_zero() {
        let backend = FakeBackend::successful();
        let (code, stdout, stderr) = run(&["gaze", "--help"], &backend);
        assert_eq!(code, EXIT_SUCCESS);
        assert!(stdout.contains("LLM-optimized screen capture CLI"));
        assert!(stderr.is_empty());
    }

    #[test]
    fn capture_defaults_to_fullscreen_json_output() {
        let backend = FakeBackend::successful();
        let (code, stdout, stderr) = run(&["gaze", "capture"], &backend);

        assert_eq!(code, EXIT_SUCCESS);
        assert!(stderr.is_empty());
        assert_eq!(
            backend.calls.borrow().as_slice(),
            ["capture_fullscreen:None"]
        );

        let payload: serde_json::Value = serde_json::from_str(&stdout).unwrap();
        assert_eq!(payload["originalWidth"], 160);
        assert!(payload["imageBase64"].as_str().unwrap().len() > 10);
        assert!(payload.get("outputPath").is_none());
    }

    #[test]
    fn capture_with_output_omits_base64_and_includes_path() {
        let backend = FakeBackend::successful();
        let dir = tempdir().unwrap();
        let output_path = dir.path().join("capture.webp");
        let output_str = output_path.display().to_string();
        let (code, stdout, _) = run(
            &[
                "gaze",
                "capture",
                "--output",
                output_str.as_str(),
                "--format",
                "json",
            ],
            &backend,
        );

        assert_eq!(code, EXIT_SUCCESS);
        assert!(output_path.exists());

        let payload: serde_json::Value = serde_json::from_str(&stdout).unwrap();
        assert!(payload.get("imageBase64").is_none());
        assert_eq!(payload["outputPath"], output_str);
    }

    #[test]
    fn capture_path_format_requires_output() {
        let backend = FakeBackend::successful();
        let (code, _, stderr) = run(&["gaze", "capture", "--format", "path"], &backend);
        assert_eq!(code, EXIT_USAGE);
        assert!(stderr.contains("--format path requires --output"));
    }

    #[test]
    fn capture_window_id_uses_direct_window_capture() {
        let backend = FakeBackend::successful();
        let (code, _, _) = run(&["gaze", "capture", "--window", "42"], &backend);
        assert_eq!(code, EXIT_SUCCESS);
        assert_eq!(backend.calls.borrow().as_slice(), ["capture_window:42"]);
    }

    #[test]
    fn capture_area_mode_uses_interactive_capture() {
        let backend = FakeBackend::successful();
        let (code, stdout, _) = run(&["gaze", "capture", "--mode", "area"], &backend);
        assert_eq!(code, EXIT_SUCCESS);
        assert_eq!(
            backend.calls.borrow().as_slice(),
            ["capture_area_interactive"]
        );

        let payload: serde_json::Value = serde_json::from_str(&stdout).unwrap();
        assert_eq!(payload["originalWidth"], 100);
        assert_eq!(payload["originalHeight"], 50);
    }

    #[test]
    fn capture_window_mode_without_window_id_is_interactive() {
        let backend = FakeBackend::successful();
        let (code, _, _) = run(&["gaze", "capture", "--mode", "window"], &backend);
        assert_eq!(code, EXIT_SUCCESS);
        assert_eq!(
            backend.calls.borrow().as_slice(),
            ["capture_window_interactive"]
        );
    }

    #[test]
    fn capture_area_cancel_returns_exit_code_3() {
        let mut backend = FakeBackend::successful();
        backend.area_capture = Ok(None);
        let (code, _, stderr) = run(&["gaze", "capture", "--mode", "area"], &backend);
        assert_eq!(code, EXIT_CANCELLED);
        assert!(stderr.contains("Capture cancelled by user"));
    }

    #[test]
    fn capture_permission_denied_returns_runtime_error() {
        let mut backend = FakeBackend::successful();
        backend.permission = false;
        let (code, _, stderr) = run(&["gaze", "capture"], &backend);
        assert_eq!(code, EXIT_FAILURE);
        assert!(stderr.contains("Screen recording permission not granted"));
    }

    #[test]
    fn capture_copy_invokes_clipboard_backend() {
        let backend = FakeBackend::successful();
        let (code, _, _) = run(&["gaze", "capture", "--copy"], &backend);
        assert_eq!(code, EXIT_SUCCESS);
        assert_eq!(
            backend.calls.borrow().as_slice(),
            ["capture_fullscreen:None", "copy:160x90"]
        );
    }

    #[test]
    fn capture_raw_keeps_original_dimensions() {
        let backend = FakeBackend::successful();
        let (code, stdout, _) = run(&["gaze", "capture", "--raw"], &backend);
        assert_eq!(code, EXIT_SUCCESS);

        let payload: serde_json::Value = serde_json::from_str(&stdout).unwrap();
        assert_eq!(payload["originalWidth"], 160);
        assert_eq!(payload["optimizedWidth"], 160);
        assert_eq!(payload["optimizedHeight"], 90);
    }

    #[test]
    fn capture_base64_format_outputs_only_base64() {
        let backend = FakeBackend::successful();
        let (code, stdout, _) = run(&["gaze", "capture", "--format", "base64"], &backend);
        assert_eq!(code, EXIT_SUCCESS);
        assert!(!stdout.contains('{'));
        assert!(stdout.trim().len() > 10);
    }

    #[test]
    fn capture_with_annotations_emits_prompt_hint_and_annotations() {
        let backend = FakeBackend::successful();
        let (code, stdout, stderr) = run(
            &[
                "gaze",
                "capture",
                "--pin",
                "80,45:broken button",
                "--rect",
                "10,10,40,20:spacing issue",
            ],
            &backend,
        );

        assert_eq!(code, EXIT_SUCCESS, "stderr: {stderr}");
        let payload: serde_json::Value = serde_json::from_str(&stdout).unwrap();
        assert!(payload["promptHint"].as_str().unwrap().contains("Pin 1"));
        assert_eq!(payload["annotations"].as_array().unwrap().len(), 2);
        assert_eq!(payload["annotations"][0]["id"], "1");
        assert_eq!(payload["annotations"][1]["id"], "A");
    }

    #[test]
    fn capture_rejects_invalid_rect_size() {
        let backend = FakeBackend::successful();
        let (code, _, stderr) = run(&["gaze", "capture", "--rect", "10,10,0,20:bad"], &backend);
        assert_eq!(code, EXIT_USAGE);
        assert!(stderr.contains("--rect width must be > 0"));
    }

    #[test]
    fn invalid_capture_argument_combo_returns_usage_error() {
        let backend = FakeBackend::successful();
        let (code, _, stderr) = run(
            &["gaze", "capture", "--mode", "area", "--window", "7"],
            &backend,
        );
        assert_eq!(code, EXIT_USAGE);
        assert!(stderr.contains("cannot be combined"));
    }

    #[test]
    fn list_displays_outputs_json() {
        let backend = FakeBackend::successful();
        let (code, stdout, _) = run(&["gaze", "list", "displays"], &backend);
        assert_eq!(code, EXIT_SUCCESS);
        assert_eq!(backend.calls.borrow().as_slice(), ["list_displays"]);

        let payload: serde_json::Value = serde_json::from_str(&stdout).unwrap();
        assert_eq!(payload[0]["name"], "Main Display");
    }

    #[test]
    fn list_windows_outputs_json() {
        let backend = FakeBackend::successful();
        let (code, stdout, _) = run(&["gaze", "list", "windows"], &backend);
        assert_eq!(code, EXIT_SUCCESS);
        assert_eq!(backend.calls.borrow().as_slice(), ["list_windows"]);

        let payload: serde_json::Value = serde_json::from_str(&stdout).unwrap();
        assert_eq!(payload[0]["title"], "Terminal");
    }

    #[test]
    fn optimize_outputs_requested_path() {
        let backend = FakeBackend::successful();
        let dir = tempdir().unwrap();
        let input_path = dir.path().join("input.png");
        let output_path = dir.path().join("output.webp");
        std::fs::write(&input_path, sample_png(200, 100)).unwrap();

        let input_str = input_path.display().to_string();
        let output_str = output_path.display().to_string();
        let (code, stdout, _) = run(
            &[
                "gaze",
                "optimize",
                input_str.as_str(),
                "--output",
                output_str.as_str(),
                "--format",
                "path",
            ],
            &backend,
        );

        assert_eq!(code, EXIT_SUCCESS);
        assert!(output_path.exists());
        assert_eq!(stdout.trim(), output_str);
    }

    #[test]
    fn optimize_can_copy_to_clipboard() {
        let backend = FakeBackend::successful();
        let dir = tempdir().unwrap();
        let input_path = dir.path().join("input.png");
        std::fs::write(&input_path, sample_png(200, 100)).unwrap();

        let input_str = input_path.display().to_string();
        let (code, _, _) = run(
            &["gaze", "optimize", input_str.as_str(), "--copy"],
            &backend,
        );

        assert_eq!(code, EXIT_SUCCESS);
        assert_eq!(backend.calls.borrow().as_slice(), ["copy:200x100"]);
    }

    #[test]
    fn optimize_missing_file_returns_runtime_error() {
        let backend = FakeBackend::successful();
        let (code, _, stderr) = run(&["gaze", "optimize", "/tmp/does-not-exist.png"], &backend);
        assert_eq!(code, EXIT_FAILURE);
        assert!(stderr.contains("Failed to read input file"));
    }

    #[test]
    fn version_subcommand_outputs_version() {
        let backend = FakeBackend::successful();
        let (code, stdout, _) = run(&["gaze", "version"], &backend);
        assert_eq!(code, EXIT_SUCCESS);
        assert_eq!(stdout.trim(), format!("gaze {}", env!("CARGO_PKG_VERSION")));
    }

    #[test]
    fn settings_get_without_file_emits_defaults() {
        let backend = FakeBackend::successful();
        let dir = tempdir().unwrap();
        let path = dir.path().join("settings.json");
        let path_str = path.display().to_string();

        let (code, stdout, stderr) = run(
            &["gaze", "settings", "--config", path_str.as_str(), "get"],
            &backend,
        );
        assert_eq!(code, EXIT_SUCCESS, "stderr: {stderr}");

        let payload: serde_json::Value = serde_json::from_str(&stdout).unwrap();
        assert_eq!(payload["language"], "en");
        assert_eq!(payload["maxPreviews"], 5);
    }

    #[test]
    fn settings_set_then_get_round_trips_through_disk() {
        let backend = FakeBackend::successful();
        let dir = tempdir().unwrap();
        let path = dir.path().join("settings.json");
        let path_str = path.display().to_string();

        let (code, _, stderr) = run(
            &[
                "gaze",
                "settings",
                "--config",
                path_str.as_str(),
                "set",
                "language",
                "ja",
            ],
            &backend,
        );
        assert_eq!(code, EXIT_SUCCESS, "stderr: {stderr}");
        assert!(path.exists());

        let (code, stdout, _) = run(
            &[
                "gaze",
                "settings",
                "--config",
                path_str.as_str(),
                "get",
                "language",
            ],
            &backend,
        );
        assert_eq!(code, EXIT_SUCCESS);
        assert_eq!(stdout.trim(), "\"ja\"");
    }

    #[test]
    fn settings_set_nested_field() {
        let backend = FakeBackend::successful();
        let dir = tempdir().unwrap();
        let path = dir.path().join("settings.json");
        let path_str = path.display().to_string();

        let (code, _, _) = run(
            &[
                "gaze",
                "settings",
                "--config",
                path_str.as_str(),
                "set",
                "maxDimension.pixels",
                "2048",
            ],
            &backend,
        );
        assert_eq!(code, EXIT_SUCCESS);

        let (_, stdout, _) = run(
            &[
                "gaze",
                "settings",
                "--config",
                path_str.as_str(),
                "get",
                "maxDimension.pixels",
            ],
            &backend,
        );
        assert_eq!(stdout.trim(), "2048");
    }

    #[test]
    fn settings_set_bool_field() {
        let backend = FakeBackend::successful();
        let dir = tempdir().unwrap();
        let path = dir.path().join("settings.json");
        let path_str = path.display().to_string();

        let (code, _, _) = run(
            &[
                "gaze",
                "settings",
                "--config",
                path_str.as_str(),
                "set",
                "autoCopy",
                "false",
            ],
            &backend,
        );
        assert_eq!(code, EXIT_SUCCESS);

        let (_, stdout, _) = run(
            &[
                "gaze",
                "settings",
                "--config",
                path_str.as_str(),
                "get",
                "autoCopy",
            ],
            &backend,
        );
        assert_eq!(stdout.trim(), "false");
    }

    #[test]
    fn settings_set_unknown_key_returns_usage_error() {
        let backend = FakeBackend::successful();
        let dir = tempdir().unwrap();
        let path = dir.path().join("settings.json");
        let path_str = path.display().to_string();

        let (code, _, stderr) = run(
            &[
                "gaze",
                "settings",
                "--config",
                path_str.as_str(),
                "set",
                "nonExistent",
                "x",
            ],
            &backend,
        );
        assert_eq!(code, EXIT_USAGE);
        assert!(stderr.contains("Unknown settings key"));
    }

    #[test]
    fn settings_set_invalid_value_type_returns_usage_error() {
        let backend = FakeBackend::successful();
        let dir = tempdir().unwrap();
        let path = dir.path().join("settings.json");
        let path_str = path.display().to_string();

        let (code, _, stderr) = run(
            &[
                "gaze",
                "settings",
                "--config",
                path_str.as_str(),
                "set",
                "maxPreviews",
                "not-a-number",
            ],
            &backend,
        );
        assert_eq!(code, EXIT_USAGE);
        assert!(stderr.contains("Invalid value"));
    }

    #[test]
    fn settings_reset_writes_defaults() {
        let backend = FakeBackend::successful();
        let dir = tempdir().unwrap();
        let path = dir.path().join("settings.json");
        let path_str = path.display().to_string();

        // Seed the file with a non-default value
        std::fs::write(&path, r#"{"language":"ja","maxPreviews":9}"#).unwrap();

        let (code, stdout, _) = run(
            &["gaze", "settings", "--config", path_str.as_str(), "reset"],
            &backend,
        );
        assert_eq!(code, EXIT_SUCCESS);
        assert!(stdout.contains("reset"));

        let (_, stdout, _) = run(
            &["gaze", "settings", "--config", path_str.as_str(), "get"],
            &backend,
        );
        let payload: serde_json::Value = serde_json::from_str(&stdout).unwrap();
        assert_eq!(payload["language"], "en");
        assert_eq!(payload["maxPreviews"], 5);
    }

    #[test]
    fn settings_get_unknown_key_returns_usage_error() {
        let backend = FakeBackend::successful();
        let dir = tempdir().unwrap();
        let path = dir.path().join("settings.json");
        let path_str = path.display().to_string();

        let (code, _, stderr) = run(
            &[
                "gaze",
                "settings",
                "--config",
                path_str.as_str(),
                "get",
                "ghost",
            ],
            &backend,
        );
        assert_eq!(code, EXIT_USAGE);
        assert!(stderr.contains("Unknown settings key"));
    }

    #[test]
    fn settings_path_prints_override_path() {
        let backend = FakeBackend::successful();
        let dir = tempdir().unwrap();
        let path = dir.path().join("settings.json");
        let path_str = path.display().to_string();

        let (code, stdout, _) = run(
            &["gaze", "settings", "--config", path_str.as_str(), "path"],
            &backend,
        );
        assert_eq!(code, EXIT_SUCCESS);
        assert_eq!(stdout.trim(), path_str);
    }

    #[test]
    fn settings_keys_lists_known_fields() {
        let backend = FakeBackend::successful();
        let (code, stdout, _) = run(&["gaze", "settings", "keys"], &backend);
        assert_eq!(code, EXIT_SUCCESS);
        // Spot-check a few canonical keys so the listing stays in sync with Settings.
        assert!(stdout.contains("language"));
        assert!(stdout.contains("launchAtLogin"));
        assert!(stdout.contains("maxDimension.pixels"));
    }

    #[test]
    fn settings_set_preserves_other_fields() {
        let backend = FakeBackend::successful();
        let dir = tempdir().unwrap();
        let path = dir.path().join("settings.json");
        let path_str = path.display().to_string();

        // Write initial settings with a known value for maxPreviews
        std::fs::write(&path, r#"{"language":"ja","maxPreviews":7}"#).unwrap();

        let (code, _, _) = run(
            &[
                "gaze",
                "settings",
                "--config",
                path_str.as_str(),
                "set",
                "language",
                "fr",
            ],
            &backend,
        );
        assert_eq!(code, EXIT_SUCCESS);

        let (_, stdout, _) = run(
            &["gaze", "settings", "--config", path_str.as_str(), "get"],
            &backend,
        );
        let payload: serde_json::Value = serde_json::from_str(&stdout).unwrap();
        assert_eq!(payload["language"], "fr");
        assert_eq!(
            payload["maxPreviews"], 7,
            "set should not clobber unrelated fields"
        );
    }

    #[test]
    fn capture_command_output_omits_base64_when_output_exists() {
        let metadata = snapforge_core::CaptureMetadata {
            original_width: 1,
            original_height: 1,
            optimized_width: 1,
            optimized_height: 1,
            file_size: 4,
            timestamp: "2026-03-31T00:00:00Z".into(),
            image_base64: "AAAA".into(),
        };
        let processed = ProcessedCapture {
            metadata,
            encoded: vec![1, 2, 3, 4],
            rgba: vec![255, 0, 0, 255],
        };

        let output =
            CaptureCommandOutput::from_processed(&processed, Some(Path::new("/tmp/x")), None, &[]);
        assert_eq!(output.image_base64, None);
        assert_eq!(output.output_path.as_deref(), Some("/tmp/x"));
    }
}
