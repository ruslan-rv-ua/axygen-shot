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

    #[error("{0}")]
    HotkeyError(String),
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
            Self::HotkeyError(_) => "hotkey-error",
        }
    }
}

/// Format error for stderr (PRD story 51)
pub fn format_error(err: &ShotError) -> String {
    format!("status: error\ncode: {}\nmessage: {}", err.code(), err)
}

/// Human-readable message only (no key:value format). Used by watch mode MessageBox.
pub fn format_error_message(err: &ShotError) -> String {
    match err {
        ShotError::WindowNotFound(s) => format!("Window not found: {}", s),
        ShotError::WindowMinimized => "Target window is minimized. Restore it and try again.".into(),
        ShotError::CaptureFailed(s) => format!("Capture failed: {}", s),
        ShotError::StorageFailed(s) => format!("Could not save screenshot: {}", s),
        ShotError::ClipboardError(s) => format!("Clipboard error: {}", s),
        ShotError::ConfigError(s) => format!("Configuration error: {}", s),
        ShotError::ArgError(s) => format!("Argument error: {}", s),
        ShotError::InitError(s) => format!("Init error: {}", s),
        ShotError::HotkeyError(s) => format!("Hotkey error: {}", s),
    }
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

    #[test]
    fn hotkey_error_code() {
        assert_eq!(ShotError::HotkeyError("x".into()).code(), "hotkey-error");
    }

    #[test]
    fn format_error_message_returns_human_readable() {
        let cases = vec![
            (ShotError::WindowNotFound("test".into()), "Window not found: test"),
            (ShotError::WindowMinimized, "Target window is minimized. Restore it and try again."),
            (ShotError::CaptureFailed("x".into()), "Capture failed: x"),
            (ShotError::StorageFailed("x".into()), "Could not save screenshot: x"),
            (ShotError::ClipboardError("x".into()), "Clipboard error: x"),
            (ShotError::ConfigError("x".into()), "Configuration error: x"),
            (ShotError::ArgError("x".into()), "Argument error: x"),
            (ShotError::InitError("x".into()), "Init error: x"),
            (ShotError::HotkeyError("x".into()), "Hotkey error: x"),
        ];
        for (err, expected) in cases {
            assert_eq!(format_error_message(&err), expected, "Failed for {:?}", err);
        }
    }
}
