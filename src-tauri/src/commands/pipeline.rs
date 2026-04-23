use crate::error::CommandError;
use snapforge_pipeline::OptimizeOptions;

#[tauri::command]
pub fn optimize_for_llm(
    image_data: Vec<u8>,
    width: u32,
    height: u32,
) -> Result<Vec<u8>, CommandError> {
    tracing::info!(width, height, "Optimizing image for LLM");
    let result = snapforge_pipeline::optimize_image(
        &image_data,
        width,
        height,
        &OptimizeOptions::default(),
    )?;
    Ok(result.encoded)
}
