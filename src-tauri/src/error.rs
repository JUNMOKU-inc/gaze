use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct CommandError {
    pub message: String,
    pub code: String,
}

impl From<snapforge_capture::CaptureError> for CommandError {
    fn from(err: snapforge_capture::CaptureError) -> Self {
        CommandError {
            message: err.to_string(),
            code: "capture_error".to_string(),
        }
    }
}

impl From<snapforge_pipeline::PipelineError> for CommandError {
    fn from(err: snapforge_pipeline::PipelineError) -> Self {
        CommandError {
            message: err.to_string(),
            code: "pipeline_error".to_string(),
        }
    }
}

impl std::fmt::Display for CommandError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[{}] {}", self.code, self.message)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_format() {
        let err = CommandError {
            message: "something failed".into(),
            code: "test_error".into(),
        };
        assert_eq!(err.to_string(), "[test_error] something failed");
    }

    #[test]
    fn from_capture_error_permission_denied() {
        let capture_err = snapforge_capture::CaptureError::PermissionDenied;
        let cmd_err = CommandError::from(capture_err);
        assert_eq!(cmd_err.code, "capture_error");
        assert_eq!(cmd_err.message, "Screen recording permission not granted");
    }

    #[test]
    fn from_capture_error_display_not_found() {
        let capture_err = snapforge_capture::CaptureError::DisplayNotFound(5);
        let cmd_err = CommandError::from(capture_err);
        assert_eq!(cmd_err.code, "capture_error");
        assert_eq!(cmd_err.message, "Display 5 not found");
    }

    #[test]
    fn from_capture_error_no_displays() {
        let capture_err = snapforge_capture::CaptureError::NoDisplays;
        let cmd_err = CommandError::from(capture_err);
        assert_eq!(cmd_err.code, "capture_error");
        assert_eq!(cmd_err.message, "No displays available");
    }

    #[test]
    fn from_pipeline_error_invalid_image() {
        let pipeline_err = snapforge_pipeline::PipelineError::InvalidImage("bad".into());
        let cmd_err = CommandError::from(pipeline_err);
        assert_eq!(cmd_err.code, "pipeline_error");
        assert_eq!(cmd_err.message, "Invalid image data: bad");
    }

    #[test]
    fn from_pipeline_error_unsupported_format() {
        let pipeline_err = snapforge_pipeline::PipelineError::UnsupportedFormat("bmp".into());
        let cmd_err = CommandError::from(pipeline_err);
        assert_eq!(cmd_err.code, "pipeline_error");
        assert_eq!(cmd_err.message, "Unsupported format: bmp");
    }

    #[test]
    fn command_error_is_serializable() {
        let err = CommandError {
            message: "test".into(),
            code: "test_code".into(),
        };
        let json = serde_json::to_string(&err).unwrap();
        assert!(json.contains("\"message\":\"test\""));
        assert!(json.contains("\"code\":\"test_code\""));
    }

    #[test]
    fn command_error_is_debug() {
        let err = CommandError {
            message: "msg".into(),
            code: "code".into(),
        };
        let debug = format!("{:?}", err);
        assert!(debug.contains("CommandError"));
    }
}
