mod audio;
mod capture;
mod cli;
mod clipboard;
mod config;
mod errors;
mod storage;
mod watch;
mod window_resolver;

use errors::ShotError;
use window_resolver::Win32Enumerator;

fn main() {
    match run() {
        Ok(()) => {}
        Err(e) => {
            audio::play_error();
            eprintln!("{}", errors::format_error(&e));
            std::process::exit(1);
        }
    }
}

fn run() -> Result<(), ShotError> {
    let args = cli::parse();
    let mode = cli::determine_mode(&args)?;

    match mode {
        cli::Mode::Init => run_init(&args),
        cli::Mode::Check => run_check(&args),
        cli::Mode::ListWindows => run_list_windows(),
        cli::Mode::Capture => run_capture(&args),
        cli::Mode::Watch => {
            let cwd = std::env::current_dir()
                .map_err(|e| ShotError::ConfigError(format!("Cannot determine CWD: {}", e)))?;
            let toml = config::find_config(&cwd)?;
            let cfg = config::merge(&args, toml)?;
            watch::run(&cfg)
        }
    }
}

fn run_init(args: &cli::CliArgs) -> Result<(), ShotError> {
    let cwd = std::env::current_dir()
        .map_err(|e| ShotError::ConfigError(format!("Cannot determine CWD: {}", e)))?;
    config::init(&cwd, args.process.as_deref(), args.title.as_deref())?;
    println!("status: ok\nfile: {}", cwd.join("shot.toml").display());
    Ok(())
}

fn run_check(args: &cli::CliArgs) -> Result<(), ShotError> {
    let cwd = std::env::current_dir()
        .map_err(|e| ShotError::ConfigError(format!("Cannot determine CWD: {}", e)))?;
    let toml = config::find_config(&cwd)?;
    let config_display = toml
        .as_ref()
        .map(|(_, root)| format!("{}", root.join("shot.toml").display()))
        .unwrap_or_else(|| "none (CLI args)".into());
    let cfg = config::merge(args, toml)?;
    let enumerator = Win32Enumerator;
    let window =
        window_resolver::resolve(&enumerator, cfg.process.as_deref(), cfg.title.as_deref())?;
    println!(
        "status: ok\nconfig: {}\nwindow: running ({}, PID {})\nfolder: {}",
        config_display,
        window.process_name,
        window.pid,
        cfg.project_root.join(&cfg.folder).display(),
    );
    Ok(())
}

fn run_list_windows() -> Result<(), ShotError> {
    let enumerator = Win32Enumerator;
    let windows = window_resolver::list_all(&enumerator);
    for w in &windows {
        println!(
            "title: {}\nprocess: {}\npid: {}\n---",
            w.title, w.process_name, w.pid
        );
    }
    Ok(())
}

fn run_capture(args: &cli::CliArgs) -> Result<(), ShotError> {
    let cwd = std::env::current_dir()
        .map_err(|e| ShotError::ConfigError(format!("Cannot determine CWD: {}", e)))?;
    let toml = config::find_config(&cwd)?;
    let cfg = config::merge(args, toml)?;

    let enumerator = Win32Enumerator;
    let window =
        window_resolver::resolve(&enumerator, cfg.process.as_deref(), cfg.title.as_deref())?;

    let result = capture::capture_window(window.hwnd)?;

    let saved = storage::save(
        &result.png_bytes,
        &cfg.project_root,
        &cfg.folder,
        cfg.label.as_deref(),
        &window.title,
    )?;

    clipboard::write_clipboard(
        cfg.clipboard,
        &saved.path,
        &result.png_bytes,
        result.width,
        result.height,
    )?;

    audio::play_success();

    if !cfg.quiet || cfg.verbose {
        println!(
            "{}",
            errors::format_success(
                &saved.path,
                &window.title,
                window.pid,
                result.width,
                result.height,
            ),
        );
    }

    Ok(())
}
