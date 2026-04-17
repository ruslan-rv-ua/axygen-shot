use clap::ValueEnum;
use serde::Deserialize;
use std::path::PathBuf;

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
