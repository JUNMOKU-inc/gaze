use serde::Serialize;

use crate::error::CommandError;
use snapforge_pipeline::{LlmProvider, OptimizeOptions};

#[derive(Debug, Serialize)]
pub struct TokenEstimate {
    pub provider: String,
    pub tokens: u32,
    pub dimensions: (u32, u32),
}

#[tauri::command]
pub fn estimate_tokens(width: u32, height: u32, provider: LlmProvider) -> TokenEstimate {
    let (opt_w, opt_h) =
        snapforge_pipeline::optimizer::calculate_optimal_dimensions(width, height, provider);
    TokenEstimate {
        provider: format!("{:?}", provider),
        tokens: provider.estimate_tokens(opt_w, opt_h),
        dimensions: (opt_w, opt_h),
    }
}

#[tauri::command]
pub fn optimize_for_llm(
    image_data: Vec<u8>,
    width: u32,
    height: u32,
    provider: LlmProvider,
) -> Result<Vec<u8>, CommandError> {
    tracing::info!(?provider, width, height, "Optimizing image for LLM");
    let options = OptimizeOptions {
        provider,
        ..Default::default()
    };
    let result = snapforge_pipeline::optimize_image(&image_data, width, height, &options)?;
    Ok(result.encoded)
}
