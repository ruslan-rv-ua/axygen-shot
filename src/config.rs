use clap::ValueEnum;
use serde::Deserialize;
use std::path::{Path, PathBuf};

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
    if folder.contains("..") {
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
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
