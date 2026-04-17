use crate::config::ClipboardMode;
use crate::errors::ShotError;
use clap::Parser;

#[derive(Parser)]
#[command(name = "shot", version, about = "Screenshot tool for developers")]
pub struct CliArgs {
    /// Optional label for the screenshot filename
    pub label: Option<String>,

    /// Target process name (e.g. "myapp.exe")
    #[arg(long)]
    pub process: Option<String>,

    /// Target window title substring
    #[arg(long)]
    pub title: Option<String>,

    /// Output subfolder (relative path)
    #[arg(long)]
    pub folder: Option<String>,

    /// Clipboard mode override
    #[arg(long, value_enum)]
    pub clipboard: Option<ClipboardMode>,

    /// Watch mode hotkey override (Phase 2)
    #[arg(long)]
    pub hotkey: Option<String>,

    /// Suppress stdout output
    #[arg(long)]
    pub quiet: bool,

    /// Print diagnostic info to stderr
    #[arg(long)]
    pub verbose: bool,

    /// Create shot.toml template in CWD
    #[arg(long)]
    pub init: bool,

    /// Watch mode (Phase 2 — returns error)
    #[arg(long)]
    pub watch: bool,

    /// Validate config and check window
    #[arg(long)]
    pub check: bool,

    /// List all visible windows
    #[arg(long)]
    pub list_windows: bool,
}

#[derive(Debug)]
pub enum Mode {
    Capture,
    Init,
    Check,
    ListWindows,
    Watch,
}

/// Parse command-line arguments
pub fn parse() -> CliArgs {
    CliArgs::parse()
}

/// Determine run mode from CLI flags. Error if multiple mode flags set.
pub fn determine_mode(args: &CliArgs) -> Result<Mode, ShotError> {
    let count = args.init as u8 + args.check as u8 + args.list_windows as u8 + args.watch as u8;
    if count > 1 {
        return Err(ShotError::ArgError(
            "Multiple mode flags set; use only one of --init, --check, --list-windows, --watch"
                .into(),
        ));
    }
    if args.init {
        return Ok(Mode::Init);
    }
    if args.check {
        return Ok(Mode::Check);
    }
    if args.list_windows {
        return Ok(Mode::ListWindows);
    }
    if args.watch {
        return Ok(Mode::Watch);
    }
    Ok(Mode::Capture)
}

// Implementation: Step 3

#[cfg(test)]
mod tests {
    use super::*;

    // --- parse tests ---

    #[test]
    fn no_args_defaults() {
        let args = CliArgs::parse_from(["shot"]);
        assert!(args.label.is_none());
        assert!(args.process.is_none());
        assert!(args.title.is_none());
        assert!(!args.quiet);
        assert!(!args.verbose);
    }

    #[test]
    fn positional_label() {
        let args = CliArgs::parse_from(["shot", "my-label"]);
        assert_eq!(args.label.as_deref(), Some("my-label"));
    }

    #[test]
    fn quoted_label_with_spaces() {
        let args = CliArgs::parse_from(["shot", "my label"]);
        assert_eq!(args.label.as_deref(), Some("my label"));
    }

    #[test]
    fn clipboard_override() {
        let args = CliArgs::parse_from(["shot", "--clipboard=image"]);
        assert_eq!(args.clipboard, Some(ClipboardMode::Image));
    }

    #[test]
    fn process_and_title_flags() {
        let args = CliArgs::parse_from(["shot", "--process=notepad.exe", "--title=Untitled"]);
        assert_eq!(args.process.as_deref(), Some("notepad.exe"));
        assert_eq!(args.title.as_deref(), Some("Untitled"));
    }

    // --- determine_mode tests ---

    #[test]
    fn default_mode_is_capture() {
        let args = CliArgs::parse_from(["shot"]);
        assert!(matches!(determine_mode(&args), Ok(Mode::Capture)));
    }

    #[test]
    fn init_mode() {
        let args = CliArgs::parse_from(["shot", "--init"]);
        assert!(matches!(determine_mode(&args), Ok(Mode::Init)));
    }

    #[test]
    fn check_mode() {
        let args = CliArgs::parse_from(["shot", "--check"]);
        assert!(matches!(determine_mode(&args), Ok(Mode::Check)));
    }

    #[test]
    fn list_windows_mode() {
        let args = CliArgs::parse_from(["shot", "--list-windows"]);
        assert!(matches!(determine_mode(&args), Ok(Mode::ListWindows)));
    }

    #[test]
    fn watch_mode() {
        let args = CliArgs::parse_from(["shot", "--watch"]);
        assert!(matches!(determine_mode(&args), Ok(Mode::Watch)));
    }

    #[test]
    fn multiple_mode_flags_error() {
        let args = CliArgs::parse_from(["shot", "--init", "--check"]);
        assert!(determine_mode(&args).is_err());
    }

    #[test]
    fn verbose_and_quiet_both_accepted() {
        // Precedence (verbose wins) is enforced in main.rs, not here
        let args = CliArgs::parse_from(["shot", "--verbose", "--quiet"]);
        assert!(args.verbose);
        assert!(args.quiet);
    }
}
