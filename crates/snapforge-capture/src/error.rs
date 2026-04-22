use thiserror::Error;

#[derive(Debug, Error)]
pub enum CaptureError {
    #[error("Screen recording permission not granted")]
    PermissionDenied,

    #[error("Display {0} not found")]
    DisplayNotFound(u32),

    #[error("No displays available")]
    NoDisplays,

    #[error("Capture failed: {0}")]
    Internal(#[from] anyhow::Error),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_permission_denied() {
        let err = CaptureError::PermissionDenied;
        assert_eq!(err.to_string(), "Screen recording permission not granted");
    }

    #[test]
    fn display_display_not_found() {
        let err = CaptureError::DisplayNotFound(42);
        assert_eq!(err.to_string(), "Display 42 not found");
    }

    #[test]
    fn display_no_displays() {
        let err = CaptureError::NoDisplays;
        assert_eq!(err.to_string(), "No displays available");
    }

    #[test]
    fn display_internal_error() {
        let inner = anyhow::anyhow!("something broke");
        let err = CaptureError::Internal(inner);
        assert_eq!(err.to_string(), "Capture failed: something broke");
    }

    #[test]
    fn from_anyhow_error() {
        let anyhow_err = anyhow::anyhow!("oops");
        let err: CaptureError = anyhow_err.into();
        assert!(matches!(err, CaptureError::Internal(_)));
    }

    #[test]
    fn error_is_debug() {
        let err = CaptureError::PermissionDenied;
        let debug = format!("{:?}", err);
        assert!(debug.contains("PermissionDenied"));
    }
}
