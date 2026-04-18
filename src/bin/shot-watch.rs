#![windows_subsystem = "windows"]

use axygen_shot::{cli, config, errors, watch};

fn main() {
    let args = cli::parse();

    // Config phase — GUI binary has no console, show error via MessageBox
    // so screen readers can announce it (MB_ICONERROR plays Critical Stop sound).
    let cfg = match (|| -> Result<config::CaptureConfig, errors::ShotError> {
        let cwd = std::env::current_dir()
            .map_err(|e| errors::ShotError::ConfigError(format!("Cannot determine CWD: {}", e)))?;
        let toml = config::find_config(&cwd)?;
        config::merge(&args, toml)
    })() {
        Ok(cfg) => cfg,
        Err(e) => {
            use windows::Win32::UI::WindowsAndMessaging::{MB_ICONERROR, MessageBoxW};
            use windows::core::PCWSTR;
            let msg = errors::format_error_message(&e);
            let msg_w: Vec<u16> = msg.encode_utf16().chain(std::iter::once(0)).collect();
            let title = "Axygen Shot \u{2014} Error";
            let title_w: Vec<u16> = title.encode_utf16().chain(std::iter::once(0)).collect();
            unsafe {
                let _ = MessageBoxW(
                    None,
                    PCWSTR(msg_w.as_ptr()),
                    PCWSTR(title_w.as_ptr()),
                    MB_ICONERROR,
                );
            }
            std::process::exit(1);
        }
    };

    // Daemon phase — run_daemon handles its own errors via handle_startup_err
    // (which already shows MessageBox when parent_pid is None).
    if watch::run_daemon(&cfg, args.daemon_parent_pid).is_err() {
        std::process::exit(1);
    }
}
