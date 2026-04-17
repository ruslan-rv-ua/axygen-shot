use std::path::Path;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ShotError {
    #[error("{0}")]
    WindowNotFound(String),

    #[error("Target window is minimized; restore it and try again")]
    WindowMinimized,

    #[error("{0}")]
    CaptureFailed(String),

    #[error("{0}")]
    StorageFailed(String),

    #[error("{0}")]
    ConfigError(String),

    #[error("{0}")]
    ClipboardError(String),

    #[error("{0}")]
    InitError(String),

    #[error("{0}")]
    ArgError(String),
}

impl ShotError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::WindowNotFound(_) => "window-not-found",
            Self::WindowMinimized => "window-minimized",
            Self::CaptureFailed(_) => "capture-failed",
            Self::StorageFailed(_) => "storage-failed",
            Self::ConfigError(_) => "config-error",
            Self::ClipboardError(_) => "clipboard-error",
            Self::InitError(_) => "init-error",
            Self::ArgError(_) => "arg-error",
        }
    }
}

/// Format error for stderr (PRD story 51)
pub fn format_error(err: &ShotError) -> String {
    format!("status: error\ncode: {}\nmessage: {}", err.code(), err)
}

/// Format success for stdout (PRD story 50)
pub fn format_success(
    file: &Path,
    window_title: &str,
    pid: u32,
    width: u32,
    height: u32,
) -> String {
    format!(
        "status: ok\nfile: {}\nwindow: {} (PID {})\nsize: {}x{}",
        file.display(),
        window_title,
        pid,
        width,
        height,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn code_window_not_found() {
        let e = ShotError::WindowNotFound("test".into());
        assert_eq!(e.code(), "window-not-found");
    }

    #[test]
    fn code_window_minimized() {
        assert_eq!(ShotError::WindowMinimized.code(), "window-minimized");
    }

    #[test]
    fn code_capture_failed() {
        assert_eq!(
            ShotError::CaptureFailed("x".into()).code(),
            "capture-failed"
        );
    }

    #[test]
    fn code_storage_failed() {
        assert_eq!(
            ShotError::StorageFailed("x".into()).code(),
            "storage-failed"
        );
    }

    #[test]
    fn code_config_error() {
        assert_eq!(ShotError::ConfigError("x".into()).code(), "config-error");
    }

    #[test]
    fn code_clipboard_error() {
        assert_eq!(
            ShotError::ClipboardError("x".into()).code(),
            "clipboard-error"
        );
    }

    #[test]
    fn code_init_error() {
        assert_eq!(ShotError::InitError("x".into()).code(), "init-error");
    }

    #[test]
    fn code_arg_error() {
        assert_eq!(ShotError::ArgError("x".into()).code(), "arg-error");
    }

    #[test]
    fn format_error_contains_all_fields() {
        let e = ShotError::CaptureFailed("gpu not available".into());
        let output = format_error(&e);
        assert!(output.contains("status: error"));
        assert!(output.contains("code: capture-failed"));
        assert!(output.contains("message: gpu not available"));
    }

    #[test]
    fn format_success_contains_all_fields() {
        let output = format_success(
            Path::new(r"C:\project\screenshots\test.png"),
            "Notepad",
            1234,
            1920,
            1080,
        );
        assert!(output.contains("status: ok"));
        assert!(output.contains(r"file: C:\project\screenshots\test.png"));
        assert!(output.contains("window: Notepad (PID 1234)"));
        assert!(output.contains("size: 1920x1080"));
    }
}
