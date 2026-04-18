use clap::ValueEnum;
use serde::Deserialize;
use std::path::{Path, PathBuf};

use crate::cli::CliArgs;
use crate::errors::ShotError;

/// Clipboard behavior — canonical definition.
/// Imported by cli.rs for clap ValueEnum derive.
#[derive(Clone, Copy, Debug, Default, PartialEq, Deserialize, ValueEnum)]
#[serde(rename_all = "lowercase")]
pub enum ClipboardMode {
    #[default]
    Path,
    Image,
    Both,
}

/// Raw TOML file representation
#[derive(Deserialize)]
pub struct TomlConfig {
    pub process: Option<String>,
    pub title: Option<String>,
    #[serde(default = "default_folder")]
    pub folder: String,
    #[serde(default)]
    pub clipboard: ClipboardMode,
    pub hotkey: Option<String>,
}

fn default_folder() -> String {
    "screenshots".to_string()
}

/// Validate folder: must be relative, no "..", no UNC, no device paths.
pub fn validate_folder(folder: &str) -> Result<(), ShotError> {
    use std::path::Component;
    if Path::new(folder)
        .components()
        .any(|c| matches!(c, Component::ParentDir))
    {
        return Err(ShotError::ConfigError(format!(
            "Folder must not contain '..': \"{}\"",
            folder
        )));
    }
    if folder.starts_with('\\') || folder.starts_with('/') || Path::new(folder).is_absolute() {
        return Err(ShotError::ConfigError(format!(
            "Folder must be a relative path: \"{}\"",
            folder
        )));
    }
    Ok(())
}

/// Walk up from start_dir looking for shot.toml.
/// Stops at .git boundary (file or directory) or filesystem root.
pub fn find_config(start_dir: &Path) -> Result<Option<(TomlConfig, PathBuf)>, ShotError> {
    let mut dir = start_dir.to_path_buf();
    loop {
        let config_path = dir.join("shot.toml");
        if config_path.exists() {
            let content = std::fs::read_to_string(&config_path).map_err(|e| {
                ShotError::ConfigError(format!("Cannot read {}: {}", config_path.display(), e))
            })?;
            let config: TomlConfig = toml::from_str(&content).map_err(|e| {
                ShotError::ConfigError(format!("Invalid TOML in {}: {}", config_path.display(), e))
            })?;
            return Ok(Some((config, dir)));
        }
        if dir.join(".git").exists() {
            return Ok(None);
        }
        if !dir.pop() {
            return Ok(None);
        }
    }
}

/// Merge CLI args with optional TOML config. CLI values override TOML.
pub fn merge(
    cli: &CliArgs,
    toml: Option<(TomlConfig, PathBuf)>,
) -> Result<CaptureConfig, ShotError> {
    let (toml_config, project_root) = match toml {
        Some((tc, root)) => (Some(tc), root),
        None => (
            None,
            std::env::current_dir()
                .map_err(|e| ShotError::ConfigError(format!("Cannot determine CWD: {}", e)))?,
        ),
    };

    let process = cli
        .process
        .clone()
        .or(toml_config.as_ref().and_then(|t| t.process.clone()));
    let title = cli
        .title
        .clone()
        .or(toml_config.as_ref().and_then(|t| t.title.clone()));

    if process.is_none() && title.is_none() {
        return Err(ShotError::ConfigError(
            "No target: provide --process or --title (or set in shot.toml)".into(),
        ));
    }

    let folder = cli.folder.clone().unwrap_or_else(|| {
        toml_config
            .as_ref()
            .map(|t| t.folder.clone())
            .unwrap_or_else(|| "screenshots".to_string())
    });
    validate_folder(&folder)?;

    let clipboard = cli.clipboard.unwrap_or_else(|| {
        toml_config
            .as_ref()
            .map(|t| t.clipboard)
            .unwrap_or_default()
    });

    let hotkey = cli
        .hotkey
        .clone()
        .or(toml_config.as_ref().and_then(|t| t.hotkey.clone()))
        .unwrap_or_else(|| "Win+F12".to_string());

    Ok(CaptureConfig {
        process,
        title,
        folder,
        clipboard,
        label: cli.label.clone(),
        quiet: cli.quiet,
        verbose: cli.verbose,
        project_root,
        hotkey,
    })
}

/// Escape special characters for TOML basic string values.
fn escape_toml_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c.is_control() => {
                use std::fmt::Write;
                let _ = write!(out, "\\u{:04X}", c as u32);
            }
            _ => out.push(c),
        }
    }
    out
}

/// Create shot.toml template. Takes dir param for testability (spec uses CWD).
pub fn init(dir: &Path, process: Option<&str>, title: Option<&str>) -> Result<(), ShotError> {
    let config_path = dir.join("shot.toml");
    if config_path.exists() {
        return Err(ShotError::InitError("shot.toml already exists".into()));
    }

    let process_line = match process {
        Some(p) => format!("process = \"{}\"", escape_toml_string(p)),
        None => "\
# process: name of your app's .exe file (recommended).\n\
# You already know this — it's in your Cargo.toml, .csproj, Makefile, etc.\n\
# process = \"myapp.exe\""
            .to_string(),
    };
    let title_line = match title {
        Some(t) => format!("title   = \"{}\"", escape_toml_string(t)),
        None => "\
# title: substring of the window title (alternative or complement to process).\n\
# Matched as \"title contains substring\" — position-independent, works with dynamic titles.\n\
# title   = \"MyApp\""
            .to_string(),
    };

    let template = format!(
        "\
# shot.toml — Axygen Shot configuration
#
# At least one of 'process' or 'title' must be uncommented.

{process_line}

{title_line}

folder    = \"screenshots\"   # output subfolder (default: \"screenshots\")
clipboard = \"path\"          # \"path\" | \"image\" | \"both\" (default: \"path\")
# hotkey  = \"Win+F12\"       # watch mode hotkey (default: \"Win+F12\")
"
    );

    std::fs::write(&config_path, &template)
        .map_err(|e| ShotError::InitError(format!("Cannot write shot.toml: {}", e)))?;

    // Append "screenshots/" to .gitignore
    let gitignore_path = dir.join(".gitignore");
    let entry = "screenshots/";
    let already_present = gitignore_path.exists() && {
        let content = std::fs::read_to_string(&gitignore_path)
            .map_err(|e| ShotError::InitError(format!("Cannot read .gitignore: {}", e)))?;
        content.lines().any(|line| line.trim() == entry)
    };

    if !already_present {
        use std::io::Write;
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&gitignore_path)
            .map_err(|e| ShotError::InitError(format!("Cannot write .gitignore: {}", e)))?;

        // Ensure we start on a new line if file doesn't end with newline
        if gitignore_path.exists() {
            let content = std::fs::read_to_string(&gitignore_path).unwrap_or_default();
            if !content.is_empty() && !content.ends_with('\n') {
                writeln!(file)
                    .map_err(|e| ShotError::InitError(format!("Cannot write .gitignore: {}", e)))?;
            }
        }

        writeln!(file, "{}", entry)
            .map_err(|e| ShotError::InitError(format!("Cannot write .gitignore: {}", e)))?;
    }

    Ok(())
}

/// Merged config — everything needed for a capture operation
pub struct CaptureConfig {
    pub process: Option<String>,
    pub title: Option<String>,
    pub folder: String,
    pub clipboard: ClipboardMode,
    pub label: Option<String>,
    pub quiet: bool,
    pub verbose: bool,
    pub project_root: PathBuf,
    pub hotkey: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::CliArgs;
    use clap::Parser;

    // --- validate_folder ---

    #[test]
    fn valid_simple_folder() {
        assert!(validate_folder("screenshots").is_ok());
    }

    #[test]
    fn valid_nested_folder() {
        assert!(validate_folder("sub/folder").is_ok());
    }

    #[test]
    fn valid_double_dot_in_name() {
        assert!(validate_folder("foo..bar").is_ok());
    }

    #[test]
    fn reject_parent_traversal() {
        assert!(validate_folder("../outside").is_err());
    }

    #[test]
    fn reject_embedded_parent_traversal() {
        assert!(validate_folder("foo/../bar").is_err());
    }

    #[test]
    fn reject_unc_path() {
        assert!(validate_folder("\\\\server\\share").is_err());
    }

    #[test]
    fn reject_device_path() {
        assert!(validate_folder("\\\\.\\pipe\\name").is_err());
    }

    #[test]
    fn reject_backslash_absolute() {
        assert!(validate_folder("\\absolute").is_err());
    }

    #[test]
    fn reject_drive_letter_absolute() {
        assert!(validate_folder("C:\\path").is_err());
    }

    #[test]
    fn reject_forward_slash_absolute() {
        assert!(validate_folder("/unix-style").is_err());
    }

    // --- find_config ---

    #[test]
    fn find_in_current_dir() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("shot.toml"), "process = \"test.exe\"\n").unwrap();
        let result = find_config(dir.path()).unwrap();
        assert!(result.is_some());
        let (config, root) = result.unwrap();
        assert_eq!(config.process.as_deref(), Some("test.exe"));
        assert_eq!(root, dir.path());
    }

    #[test]
    fn find_in_parent_dir() {
        let parent = tempfile::tempdir().unwrap();
        let child = parent.path().join("subdir");
        std::fs::create_dir(&child).unwrap();
        std::fs::write(parent.path().join("shot.toml"), "process = \"test.exe\"\n").unwrap();
        let result = find_config(&child).unwrap();
        assert!(result.is_some());
        let (_, root) = result.unwrap();
        assert_eq!(root, parent.path());
    }

    #[test]
    fn stops_at_git_directory() {
        let root = tempfile::tempdir().unwrap();
        let child = root.path().join("child");
        let grandchild = child.join("grandchild");
        std::fs::create_dir_all(&grandchild).unwrap();
        std::fs::write(root.path().join("shot.toml"), "process = \"test.exe\"\n").unwrap();
        std::fs::create_dir(child.join(".git")).unwrap();
        let result = find_config(&grandchild).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn stops_at_git_file() {
        let root = tempfile::tempdir().unwrap();
        let child = root.path().join("child");
        std::fs::create_dir(&child).unwrap();
        std::fs::write(root.path().join("shot.toml"), "process = \"test.exe\"\n").unwrap();
        std::fs::write(child.join(".git"), "gitdir: ../other\n").unwrap();
        let result = find_config(&child).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn not_found_returns_none() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join(".git")).unwrap();
        let result = find_config(dir.path()).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn invalid_toml_returns_error() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("shot.toml"), "not valid toml [[[").unwrap();
        assert!(find_config(dir.path()).is_err());
    }

    #[test]
    fn unknown_keys_ignored() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("shot.toml"),
            "process = \"test.exe\"\nunknown_key = \"value\"\n",
        )
        .unwrap();
        assert!(find_config(dir.path()).unwrap().is_some());
    }

    // --- merge ---

    #[test]
    fn merge_cli_overrides_toml_process() {
        let cli = CliArgs::parse_from(["shot", "--process=cli.exe"]);
        let toml_cfg = TomlConfig {
            process: Some("toml.exe".into()),
            title: None,
            folder: "screenshots".into(),
            clipboard: ClipboardMode::Path,
            hotkey: None,
        };
        let dir = tempfile::tempdir().unwrap();
        let config = merge(&cli, Some((toml_cfg, dir.path().to_path_buf()))).unwrap();
        assert_eq!(config.process.as_deref(), Some("cli.exe"));
    }

    #[test]
    fn merge_toml_provides_defaults() {
        let cli = CliArgs::parse_from(["shot"]);
        let toml_cfg = TomlConfig {
            process: Some("toml.exe".into()),
            title: None,
            folder: "output".into(),
            clipboard: ClipboardMode::Image,
            hotkey: None,
        };
        let dir = tempfile::tempdir().unwrap();
        let config = merge(&cli, Some((toml_cfg, dir.path().to_path_buf()))).unwrap();
        assert_eq!(config.process.as_deref(), Some("toml.exe"));
        assert_eq!(config.folder, "output");
        assert_eq!(config.clipboard, ClipboardMode::Image);
    }

    #[test]
    fn merge_no_target_with_config_errors() {
        let cli = CliArgs::parse_from(["shot"]);
        let toml_cfg = TomlConfig {
            process: None,
            title: None,
            folder: "screenshots".into(),
            clipboard: ClipboardMode::Path,
            hotkey: None,
        };
        let dir = tempfile::tempdir().unwrap();
        assert!(merge(&cli, Some((toml_cfg, dir.path().to_path_buf()))).is_err());
    }

    #[test]
    fn merge_no_target_no_config_errors() {
        let cli = CliArgs::parse_from(["shot"]);
        assert!(merge(&cli, None).is_err());
    }

    #[test]
    fn merge_cli_folder_overrides_toml() {
        let cli = CliArgs::parse_from(["shot", "--process=test.exe", "--folder=custom"]);
        let toml_cfg = TomlConfig {
            process: None,
            title: None,
            folder: "screenshots".into(),
            clipboard: ClipboardMode::Path,
            hotkey: None,
        };
        let dir = tempfile::tempdir().unwrap();
        let config = merge(&cli, Some((toml_cfg, dir.path().to_path_buf()))).unwrap();
        assert_eq!(config.folder, "custom");
    }

    #[test]
    fn merge_invalid_folder_errors() {
        let cli = CliArgs::parse_from(["shot", "--process=test.exe", "--folder=../bad"]);
        assert!(merge(&cli, None).is_err());
    }

    #[test]
    fn merge_default_clipboard_is_path() {
        let cli = CliArgs::parse_from(["shot", "--process=test.exe"]);
        let config = merge(&cli, None).unwrap();
        assert_eq!(config.clipboard, ClipboardMode::Path);
    }

    // --- init ---

    #[test]
    fn init_creates_shot_toml() {
        let dir = tempfile::tempdir().unwrap();
        init(dir.path(), None, None).unwrap();
        let path = dir.path().join("shot.toml");
        assert!(path.exists());
        let content = std::fs::read_to_string(path).unwrap();
        assert!(content.contains("folder"));
        assert!(content.contains("clipboard"));
    }

    #[test]
    fn init_with_process() {
        let dir = tempfile::tempdir().unwrap();
        init(dir.path(), Some("myapp.exe"), None).unwrap();
        let content = std::fs::read_to_string(dir.path().join("shot.toml")).unwrap();
        assert!(content.lines().any(|l| l == r#"process = "myapp.exe""#));
    }

    #[test]
    fn init_with_title() {
        let dir = tempfile::tempdir().unwrap();
        init(dir.path(), None, Some("MyApp")).unwrap();
        let content = std::fs::read_to_string(dir.path().join("shot.toml")).unwrap();
        assert!(content.lines().any(|l| l == r#"title   = "MyApp""#));
    }

    #[test]
    fn init_fails_if_exists() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("shot.toml"), "existing").unwrap();
        assert!(init(dir.path(), None, None).is_err());
    }

    #[test]
    fn init_creates_gitignore() {
        let dir = tempfile::tempdir().unwrap();
        init(dir.path(), None, None).unwrap();
        let content = std::fs::read_to_string(dir.path().join(".gitignore")).unwrap();
        assert!(content.contains("screenshots/"));
    }

    #[test]
    fn init_appends_to_existing_gitignore() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join(".gitignore"), "node_modules/\n").unwrap();
        init(dir.path(), None, None).unwrap();
        let content = std::fs::read_to_string(dir.path().join(".gitignore")).unwrap();
        assert!(content.contains("node_modules/"));
        assert!(content.contains("screenshots/"));
    }

    #[test]
    fn init_skips_duplicate_gitignore_entry() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join(".gitignore"), "screenshots/\n").unwrap();
        init(dir.path(), None, None).unwrap();
        let content = std::fs::read_to_string(dir.path().join(".gitignore")).unwrap();
        assert_eq!(content.matches("screenshots/").count(), 1);
    }

    #[test]
    fn init_appends_newline_before_entry_if_missing() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join(".gitignore"), "node_modules/").unwrap(); // no trailing \n
        init(dir.path(), None, None).unwrap();
        let content = std::fs::read_to_string(dir.path().join(".gitignore")).unwrap();
        assert!(content.contains("node_modules/\nscreenshots/"));
    }

    #[test]
    fn init_template_is_valid_toml() {
        let dir = tempfile::tempdir().unwrap();
        init(dir.path(), Some("myapp.exe"), Some("MyApp")).unwrap();
        let content = std::fs::read_to_string(dir.path().join("shot.toml")).unwrap();
        let parsed: TomlConfig = toml::from_str(&content).unwrap();
        assert_eq!(parsed.process.as_deref(), Some("myapp.exe"));
        assert_eq!(parsed.title.as_deref(), Some("MyApp"));
    }

    #[test]
    fn merge_hotkey_default() {
        let cli = CliArgs::parse_from(["shot", "--process=x.exe"]);
        let cfg = merge(&cli, None).unwrap();
        assert_eq!(cfg.hotkey, "Win+F12");
    }

    #[test]
    fn merge_hotkey_from_toml() {
        let toml_str = "process = \"x.exe\"\nhotkey = \"Ctrl+F5\"";
        let toml_config: TomlConfig = toml::from_str(toml_str).unwrap();
        let cli = CliArgs::parse_from(["shot"]);
        let cfg = merge(&cli, Some((toml_config, PathBuf::from(".")))).unwrap();
        assert_eq!(cfg.hotkey, "Ctrl+F5");
    }

    #[test]
    fn merge_hotkey_cli_overrides_toml() {
        let toml_str = "process = \"x.exe\"\nhotkey = \"Ctrl+F5\"";
        let toml_config: TomlConfig = toml::from_str(toml_str).unwrap();
        let cli = CliArgs::parse_from(["shot", "--hotkey=Win+F11"]);
        let cfg = merge(&cli, Some((toml_config, PathBuf::from(".")))).unwrap();
        assert_eq!(cfg.hotkey, "Win+F11");
    }

    // --- escape_toml_string ---

    #[test]
    fn escape_toml_backslash_and_quote() {
        assert_eq!(escape_toml_string(r#"a\"b"#), r#"a\\\"b"#);
    }

    #[test]
    fn escape_toml_control_chars() {
        assert_eq!(escape_toml_string("a\nb\rc\td"), "a\\nb\\rc\\td");
    }

    #[test]
    fn escape_toml_null_byte() {
        assert_eq!(escape_toml_string("a\0b"), "a\\u0000b");
    }

    #[test]
    fn escape_toml_newline_injection_produces_valid_toml() {
        let malicious = "evil\n[malicious]\nprocess = \"other.exe\"";
        let escaped = escape_toml_string(malicious);
        let toml_str = format!("process = \"{}\"", escaped);
        let parsed: TomlConfig = toml::from_str(&toml_str).unwrap();
        assert_eq!(parsed.process.as_deref(), Some(malicious));
    }
}
