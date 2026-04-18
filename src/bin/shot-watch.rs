#![windows_subsystem = "windows"]

use axygen_shot::{cli, config, errors, watch};

fn main() {
    let args = cli::parse();
    let result = (|| -> Result<(), errors::ShotError> {
        let cwd = std::env::current_dir()
            .map_err(|e| errors::ShotError::ConfigError(format!("Cannot determine CWD: {}", e)))?;
        let toml = config::find_config(&cwd)?;
        let cfg = config::merge(&args, toml)?;
        watch::run_daemon(&cfg, args.daemon_parent_pid)
    })();
    if result.is_err() {
        std::process::exit(1);
    }
}
