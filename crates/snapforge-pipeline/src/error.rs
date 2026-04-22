use thiserror::Error;

#[derive(Debug, Error)]
pub enum PipelineError {
    #[error("Invalid image data: {0}")]
    InvalidImage(String),

    #[error("Unsupported format: {0}")]
    UnsupportedFormat(String),

    #[error("Image processing failed: {0}")]
    ProcessingFailed(#[from] image::ImageError),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_invalid_image() {
        let err = PipelineError::InvalidImage("bad data".into());
        assert_eq!(err.to_string(), "Invalid image data: bad data");
    }

    #[test]
    fn display_unsupported_format() {
        let err = PipelineError::UnsupportedFormat("bmp".into());
        assert_eq!(err.to_string(), "Unsupported format: bmp");
    }

    #[test]
    fn display_processing_failed() {
        // Create an ImageError by trying to decode invalid data
        let bad_data = std::io::Cursor::new(vec![0u8; 10]);
        let img_err = image::ImageReader::with_format(bad_data, image::ImageFormat::Png)
            .decode()
            .unwrap_err();
        let err = PipelineError::from(img_err);
        let msg = err.to_string();
        assert!(
            msg.starts_with("Image processing failed:"),
            "unexpected: {msg}"
        );
    }

    #[test]
    fn error_is_debug() {
        let err = PipelineError::InvalidImage("test".into());
        let debug = format!("{:?}", err);
        assert!(debug.contains("InvalidImage"));
    }
}
