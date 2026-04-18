use crate::config::CaptureConfig;
use crate::errors::ShotError;
use crate::window_resolver::Win32Enumerator;
use crate::{audio, capture, clipboard, errors, storage, window_resolver};

use std::cell::Cell;
use std::ffi::OsString;
use std::mem;
use std::os::windows::ffi::OsStrExt;
use windows::Win32::Foundation::{
    ERROR_ACCESS_DENIED, ERROR_ALREADY_EXISTS, HANDLE, HWND, LPARAM, LRESULT, POINT, WPARAM,
};
use windows::Win32::System::Console::GetConsoleWindow;
use windows::Win32::System::Threading::{
    CreateEventW, CreateMutexW, CreateProcessW, EVENT_MODIFY_STATE, GetCurrentProcessId,
    OpenEventW, PROCESS_CREATION_FLAGS, PROCESS_INFORMATION, STARTUPINFOW, SetEvent,
    WaitForMultipleObjects,
};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    HOT_KEY_MODIFIERS, RegisterHotKey, UnregisterHotKey,
};
use windows::Win32::UI::Shell::{
    NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE, NOTIFYICONDATAW, Shell_NotifyIconW,
};
use windows::Win32::UI::WindowsAndMessaging::{
    AppendMenuW, CreatePopupMenu, CreateWindowExW, DefWindowProcW, DestroyMenu, DestroyWindow,
    DispatchMessageW, GWLP_USERDATA, GetCursorPos, GetMessageW, GetWindowLongPtrW, HWND_MESSAGE,
    IDI_APPLICATION, LoadIconW, MB_ICONERROR, MENU_ITEM_FLAGS, MESSAGEBOX_STYLE, MSG, MessageBoxW,
    PostMessageW, PostQuitMessage, RegisterClassExW, SetForegroundWindow, SetWindowLongPtrW,
    TPM_RIGHTALIGN, TrackPopupMenu, TranslateMessage, WINDOW_EX_STYLE, WINDOW_STYLE, WM_APP,
    WM_COMMAND, WM_HOTKEY, WM_NULL, WM_RBUTTONUP, WNDCLASSEXW,
};
use windows::core::{PCWSTR, PWSTR, w};

const CREATE_NEW_PROCESS_GROUP: u32 = 0x00000200;
const DETACHED_PROCESS: u32 = 0x00000008;
const CREATE_NO_WINDOW: u32 = 0x08000000;

// Win32 hotkey modifier flags
const MOD_ALT: u32 = 0x0001;
const MOD_CONTROL: u32 = 0x0002;
const MOD_SHIFT: u32 = 0x0004;
const MOD_WIN: u32 = 0x0008;
const MOD_NOREPEAT: u32 = 0x4000;

// Daemon constants
const WM_TRAY_CALLBACK: u32 = WM_APP + 1;
const ID_EXIT: usize = 1001;
const ID_RESTART: usize = 1002;
const HOTKEY_ID: i32 = 1;

struct DaemonState {
    cfg: CaptureConfig,
    capturing: Cell<bool>,
    mutex_handle: HANDLE,
}

fn temp_file_path(parent_pid: u32) -> std::path::PathBuf {
    let mut path = std::env::temp_dir();
    path.push(format!("shot-err-{}.txt", parent_pid));
    path
}

fn write_temp_file(parent_pid: u32, code: &str, message: &str) {
    let path = temp_file_path(parent_pid);
    let _ = std::fs::write(&path, format!("{}\n{}", code, message));
}

fn read_and_delete_temp_file(parent_pid: u32) -> ShotError {
    let path = temp_file_path(parent_pid);
    let content = std::fs::read_to_string(&path).unwrap_or_default();
    let _ = std::fs::remove_file(&path);
    let mut lines = content.lines();
    let code = lines.next().unwrap_or("");
    let message = lines.next().unwrap_or("daemon failed to start");
    match code {
        "watch-already-running" => ShotError::WatchAlreadyRunning,
        "hotkey-error" => ShotError::HotkeyError(message.to_string()),
        "tray-error" => ShotError::TrayError(message.to_string()),
        _ => ShotError::HotkeyError(format!("daemon failed to start: {}", message)),
    }
}

/// Parse a hotkey string like "Win+F12" into (modifiers, virtual_key_code).
/// Modifiers are OR'd together. MOD_NOREPEAT is always added.
/// At least one modifier required. Exactly one key required.
pub fn parse_hotkey(s: &str) -> Result<(u32, u32), ShotError> {
    let mut modifiers: u32 = 0;
    let mut vk: Option<u32> = None;

    for token in s.split('+') {
        let token = token.trim();
        if token.is_empty() {
            continue;
        }
        match token.to_lowercase().as_str() {
            "win" => modifiers |= MOD_WIN,
            "ctrl" | "control" => modifiers |= MOD_CONTROL,
            "shift" => modifiers |= MOD_SHIFT,
            "alt" => modifiers |= MOD_ALT,
            key => {
                if vk.is_some() {
                    return Err(ShotError::HotkeyError(format!(
                        "Invalid hotkey '{}': multiple keys specified",
                        s
                    )));
                }
                vk = Some(parse_vk(key, s)?);
            }
        }
    }

    let vk = vk.ok_or_else(|| {
        ShotError::HotkeyError(format!("Invalid hotkey '{}': no key specified", s))
    })?;

    if modifiers == 0 {
        return Err(ShotError::HotkeyError(format!(
            "Invalid hotkey '{}': at least one modifier required (Win, Ctrl, Shift, Alt)",
            s,
        )));
    }

    Ok((modifiers | MOD_NOREPEAT, vk))
}

fn parse_vk(key: &str, full_hotkey: &str) -> Result<u32, ShotError> {
    let lower = key.to_lowercase();

    // F1–F24
    if let Some(rest) = lower.strip_prefix('f')
        && let Ok(n) = rest.parse::<u32>()
        && (1..=24).contains(&n)
    {
        return Ok(0x6F + n); // VK_F1 = 0x70
    }

    // Single character: A–Z or 0–9
    if lower.len() == 1 {
        let ch = lower.as_bytes()[0];
        if ch.is_ascii_lowercase() {
            return Ok((ch - b'a' + b'A') as u32);
        }
        if ch.is_ascii_digit() {
            return Ok(ch as u32);
        }
    }

    // Named keys
    match lower.as_str() {
        "printscreen" | "prtsc" => Ok(0x2C), // VK_SNAPSHOT
        _ => Err(ShotError::HotkeyError(format!(
            "Invalid hotkey key '{}' in '{}' — valid keys: F1-F24, A-Z, 0-9, PrintScreen",
            key, full_hotkey,
        ))),
    }
}

pub fn run(cfg: &CaptureConfig, parent_pid: Option<u32>) -> Result<(), ShotError> {
    if !is_detached() {
        let my_pid = unsafe { GetCurrentProcessId() };

        let ok_name = wide_string(&format!("Local\\axygen-shot-ok-{}", my_pid));
        let err_name = wide_string(&format!("Local\\axygen-shot-err-{}", my_pid));

        let ok_event = unsafe {
            CreateEventW(None, false, false, windows::core::PCWSTR(ok_name.as_ptr()))
                .map_err(|e| ShotError::HotkeyError(format!("Cannot create ok event: {}", e)))?
        };
        let err_event = unsafe {
            CreateEventW(None, false, false, windows::core::PCWSTR(err_name.as_ptr())).map_err(
                |e| {
                    let _ = windows::Win32::Foundation::CloseHandle(ok_event);
                    ShotError::HotkeyError(format!("Cannot create err event: {}", e))
                },
            )?
        };

        let child_pid = match launch_daemon(my_pid) {
            Ok(pid) => pid,
            Err(e) => {
                unsafe {
                    let _ = windows::Win32::Foundation::CloseHandle(ok_event);
                    let _ = windows::Win32::Foundation::CloseHandle(err_event);
                }
                return Err(e);
            }
        };

        // Block up to 3 s for the daemon to signal ok (index 0) or err (index 1).
        let wait_result = unsafe { WaitForMultipleObjects(&[ok_event, err_event], false, 3000) };

        unsafe {
            let _ = windows::Win32::Foundation::CloseHandle(ok_event);
            let _ = windows::Win32::Foundation::CloseHandle(err_event);
        }

        return match wait_result.0 {
            0 => {
                if !cfg.quiet || cfg.verbose {
                    println!("status: ok\nwatch: started (PID {})", child_pid);
                }
                Ok(())
            }
            1 => Err(read_and_delete_temp_file(my_pid)),
            _ => Err(ShotError::WatchTimeout),
        };
    }
    run_daemon(cfg, parent_pid)
}

fn is_detached() -> bool {
    unsafe { GetConsoleWindow().0.is_null() }
}

// Windows CommandLineToArgvW-compatible quoting: doubles backslashes before quotes/end.
fn quote_arg(arg: &str) -> String {
    let mut s = String::from('"');
    let mut backslashes = 0usize;
    for c in arg.chars() {
        match c {
            '\\' => backslashes += 1,
            '"' => {
                for _ in 0..=backslashes {
                    s.push('\\');
                }
                backslashes = 0;
                s.push('"');
                continue;
            }
            _ => backslashes = 0,
        }
        s.push(c);
    }
    // Double trailing backslashes so the closing quote isn't accidentally escaped.
    for _ in 0..backslashes {
        s.push('\\');
    }
    s.push('"');
    s
}

fn launch_daemon(parent_pid: u32) -> Result<u32, ShotError> {
    let exe = std::env::current_exe()
        .map_err(|e| ShotError::HotkeyError(format!("Cannot find own executable: {}", e)))?;

    let mut cmd_line = OsString::new();
    cmd_line.push("\"");
    cmd_line.push(exe.as_os_str());
    cmd_line.push("\"");
    for arg in std::env::args().skip(1) {
        cmd_line.push(" ");
        cmd_line.push(quote_arg(&arg).as_str());
    }
    let pid_arg = format!("--daemon-parent-pid={}", parent_pid);
    cmd_line.push(" ");
    cmd_line.push(pid_arg.as_str());
    let mut cmd_wide: Vec<u16> = cmd_line.encode_wide().chain(std::iter::once(0)).collect();

    let flags =
        PROCESS_CREATION_FLAGS(CREATE_NEW_PROCESS_GROUP | DETACHED_PROCESS | CREATE_NO_WINDOW);
    let si = STARTUPINFOW {
        cb: std::mem::size_of::<STARTUPINFOW>() as u32,
        ..Default::default()
    };
    let mut pi = PROCESS_INFORMATION::default();

    unsafe {
        CreateProcessW(
            None,
            Some(PWSTR(cmd_wide.as_mut_ptr())),
            None,
            None,
            false,
            flags,
            None,
            None,
            &si,
            &mut pi,
        )
        .map_err(|e| ShotError::HotkeyError(format!("Cannot launch daemon: {}", e)))?;

        let _ = windows::Win32::Foundation::CloseHandle(pi.hProcess);
        let _ = windows::Win32::Foundation::CloseHandle(pi.hThread);

        Ok(pi.dwProcessId)
    }
}

/// Opens the parent's ok/err named events (if `parent_pid` is set).
/// Returns `(ok_event, err_event)` — both will be null if no parent.
fn open_parent_events(parent_pid: Option<u32>) -> (HANDLE, HANDLE) {
    let pid = match parent_pid {
        Some(p) => p,
        None => return (HANDLE::default(), HANDLE::default()),
    };
    let ok_name = wide_string(&format!("Local\\axygen-shot-ok-{pid}"));
    let err_name = wide_string(&format!("Local\\axygen-shot-err-{pid}"));
    unsafe {
        let ok = OpenEventW(
            EVENT_MODIFY_STATE,
            false,
            windows::core::PCWSTR(ok_name.as_ptr()),
        )
        .unwrap_or_default();
        let err = OpenEventW(
            EVENT_MODIFY_STATE,
            false,
            windows::core::PCWSTR(err_name.as_ptr()),
        )
        .unwrap_or_default();
        (ok, err)
    }
}

/// Signals the parent's ok event (no-op if handle is null).
fn signal_ok(ok_event: HANDLE) {
    if !ok_event.0.is_null() {
        unsafe {
            let _ = SetEvent(ok_event);
        }
    }
}

/// Writes the error to a temp file, signals the err event, then exits.
/// Falls back to a message box when there is no parent (manual or restart path).
fn handle_startup_err(err_event: HANDLE, parent_pid: Option<u32>, err: &ShotError) {
    if let Some(pid) = parent_pid {
        write_temp_file(pid, err.code(), &errors::format_error_message(err));
        if !err_event.0.is_null() {
            unsafe {
                let _ = SetEvent(err_event);
            }
        }
        std::process::exit(1);
    } else {
        message_box(
            &errors::format_error_message(err),
            "Axygen Shot — Error",
            MB_ICONERROR,
        );
    }
}

fn run_daemon(cfg: &CaptureConfig, parent_pid: Option<u32>) -> Result<(), ShotError> {
    let (ok_event, err_event) = open_parent_events(parent_pid);

    // Single-instance guard: only one daemon may run system-wide.
    let mutex_name = wide_string("Global\\axygen-shot-daemon");
    let mutex_handle = unsafe {
        match CreateMutexW(None, true, windows::core::PCWSTR(mutex_name.as_ptr())) {
            Ok(h) => {
                // Call GetLastError IMMEDIATELY — before any other Win32 call.
                let last_err = windows::Win32::Foundation::GetLastError();
                if last_err == ERROR_ALREADY_EXISTS || last_err == ERROR_ACCESS_DENIED {
                    let err = ShotError::WatchAlreadyRunning;
                    handle_startup_err(err_event, parent_pid, &err);
                    return Err(err);
                }
                h
            }
            Err(e) => {
                let err = ShotError::HotkeyError(format!("Cannot create instance mutex: {}", e));
                handle_startup_err(err_event, parent_pid, &err);
                return Err(err);
            }
        }
    };

    // Parse hotkey before creating any windows
    let (modifiers, vk) = match parse_hotkey(&cfg.hotkey) {
        Ok(v) => v,
        Err(e) => {
            handle_startup_err(err_event, parent_pid, &e);
            return Err(e);
        }
    };

    // Register window class
    let class_name = wide_string("ShotWatchClass");
    let wc = WNDCLASSEXW {
        cbSize: mem::size_of::<WNDCLASSEXW>() as u32,
        lpfnWndProc: Some(wndproc),
        lpszClassName: PCWSTR(class_name.as_ptr()),
        ..Default::default()
    };
    unsafe { RegisterClassExW(&wc) };

    // Create hidden message-only window
    let hwnd = unsafe {
        CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            PCWSTR(class_name.as_ptr()),
            PCWSTR::null(),
            WINDOW_STYLE::default(),
            0,
            0,
            0,
            0,
            Some(HWND_MESSAGE),
            None,
            None,
            None,
        )
    };
    let hwnd = match hwnd {
        Ok(h) if !h.0.is_null() => h,
        _ => {
            let err = ShotError::HotkeyError("Cannot create message window".into());
            handle_startup_err(err_event, parent_pid, &err);
            return Err(err);
        }
    };

    // Store daemon state via GWLP_USERDATA for wndproc access
    let state = Box::new(DaemonState {
        cfg: cfg.clone(),
        capturing: Cell::new(false),
        mutex_handle,
    });
    unsafe { SetWindowLongPtrW(hwnd, GWLP_USERDATA, Box::into_raw(state) as isize) };

    // Register global hotkey
    let hotkey_result =
        unsafe { RegisterHotKey(Some(hwnd), HOTKEY_ID, HOT_KEY_MODIFIERS(modifiers), vk) };
    if let Err(e) = hotkey_result {
        unsafe {
            let _ = DestroyWindow(hwnd);
        }
        let err =
            ShotError::HotkeyError(format!("RegisterHotKey failed for '{}': {}", cfg.hotkey, e));
        handle_startup_err(err_event, parent_pid, &err);
        return Err(err);
    }

    // Add tray icon
    let tooltip = build_tooltip(cfg);
    let hicon = unsafe { LoadIconW(None, IDI_APPLICATION).unwrap_or_default() };
    let mut nid = NOTIFYICONDATAW {
        cbSize: mem::size_of::<NOTIFYICONDATAW>() as u32,
        hWnd: hwnd,
        uID: 1,
        uFlags: NIF_MESSAGE | NIF_ICON | NIF_TIP,
        uCallbackMessage: WM_TRAY_CALLBACK,
        hIcon: hicon,
        ..Default::default()
    };
    copy_to_wide_buf(&tooltip, &mut nid.szTip);

    let tray_ok = unsafe { Shell_NotifyIconW(NIM_ADD, &nid) };
    if !tray_ok.as_bool() {
        unsafe {
            let _ = UnregisterHotKey(Some(hwnd), HOTKEY_ID);
            let _ = DestroyWindow(hwnd);
        }
        let err = ShotError::TrayError(
            "cannot create tray icon — system tray may not be available".into(),
        );
        handle_startup_err(err_event, parent_pid, &err);
        return Err(err);
    }

    // Signal the parent that daemon started successfully.
    signal_ok(ok_event);
    // Close event handles — no longer needed after signaling.
    unsafe {
        if !ok_event.0.is_null() {
            let _ = windows::Win32::Foundation::CloseHandle(ok_event);
        }
        if !err_event.0.is_null() {
            let _ = windows::Win32::Foundation::CloseHandle(err_event);
        }
    }

    // Play startup sound (PRD story 59)
    audio::play_startup();

    // Message loop
    unsafe {
        let mut msg: MSG = mem::zeroed();
        loop {
            match GetMessageW(&mut msg, None, 0, 0).0 {
                -1 | 0 => break,
                _ => {
                    let _ = TranslateMessage(&msg);
                    DispatchMessageW(&msg);
                }
            }
        }

        let _ = Shell_NotifyIconW(NIM_DELETE, &nid);
        let _ = UnregisterHotKey(Some(hwnd), HOTKEY_ID);
        // Free DaemonState allocated via Box::into_raw
        let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA);
        if ptr != 0 {
            drop(Box::from_raw(ptr as *mut DaemonState));
        }
    }

    Ok(())
}

unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        WM_HOTKEY => {
            let ptr = unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) };
            if ptr != 0 {
                let state = unsafe { &*(ptr as *const DaemonState) };
                if state.capturing.get() {
                    audio::play_busy();
                } else {
                    state.capturing.set(true);
                    do_capture(&state.cfg);
                    state.capturing.set(false);
                }
            }
            LRESULT(0)
        }
        m if m == WM_TRAY_CALLBACK => {
            let mouse_msg = (lparam.0 & 0xFFFF) as u32;
            if mouse_msg == WM_RBUTTONUP {
                show_context_menu(hwnd);
            }
            LRESULT(0)
        }
        WM_COMMAND => {
            let id = wparam.0 & 0xFFFF;
            match id {
                ID_EXIT => {
                    unsafe { PostQuitMessage(0) };
                }
                ID_RESTART => {
                    let ptr = unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) };
                    if ptr != 0 {
                        let state = unsafe { &*(ptr as *const DaemonState) };
                        // Release mutex BEFORE spawning new daemon to avoid race
                        unsafe {
                            let _ = windows::Win32::Foundation::CloseHandle(state.mutex_handle);
                        }
                    }
                    restart();
                    unsafe { PostQuitMessage(0) };
                }
                _ => {}
            }
            LRESULT(0)
        }
        _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
}

fn do_capture(cfg: &CaptureConfig) {
    let result = (|| -> Result<(), ShotError> {
        let enumerator = Win32Enumerator;
        let window =
            window_resolver::resolve(&enumerator, cfg.process.as_deref(), cfg.title.as_deref())?;
        let capture_result = capture::capture_window(window.hwnd)?;
        let saved = storage::save(
            &capture_result.png_bytes,
            &cfg.project_root,
            &cfg.folder,
            None,
            &window.title,
        )?;
        clipboard::write_clipboard(
            cfg.clipboard,
            &saved.path,
            &capture_result.bgra_pixels,
            capture_result.width,
            capture_result.height,
        )?;
        Ok(())
    })();

    match result {
        Ok(()) => audio::play_success(),
        Err(e) => {
            audio::play_error();
            let msg = errors::format_error_message(&e);
            message_box(&msg, "Axygen Shot — Error", MB_ICONERROR);
        }
    }
}

fn show_context_menu(hwnd: HWND) {
    unsafe {
        let Ok(menu) = CreatePopupMenu() else { return };
        let _ = AppendMenuW(menu, MENU_ITEM_FLAGS(0), ID_RESTART, w!("Restart"));
        let _ = AppendMenuW(menu, MENU_ITEM_FLAGS(0), ID_EXIT, w!("Exit"));

        let mut pt = POINT::default();
        let _ = GetCursorPos(&mut pt);
        let _ = SetForegroundWindow(hwnd);
        let _ = TrackPopupMenu(menu, TPM_RIGHTALIGN, pt.x, pt.y, Some(0), hwnd, None);
        let _ = PostMessageW(Some(hwnd), WM_NULL, WPARAM(0), LPARAM(0));
        let _ = DestroyMenu(menu);
    }
}

fn restart() {
    let exe = std::env::current_exe().ok();
    if let Some(exe) = exe {
        let mut cmd_line = OsString::new();
        cmd_line.push("\"");
        cmd_line.push(exe.as_os_str());
        cmd_line.push("\"");
        for arg in std::env::args().skip(1) {
            if arg.starts_with("--daemon-parent-pid") {
                continue; // Internal IPC arg — must not be forwarded to restarted daemon
            }
            cmd_line.push(" ");
            cmd_line.push(quote_arg(&arg).as_str());
        }
        let mut cmd_wide: Vec<u16> = cmd_line.encode_wide().chain(std::iter::once(0)).collect();
        let flags =
            PROCESS_CREATION_FLAGS(CREATE_NEW_PROCESS_GROUP | DETACHED_PROCESS | CREATE_NO_WINDOW);
        let si = STARTUPINFOW {
            cb: mem::size_of::<STARTUPINFOW>() as u32,
            ..Default::default()
        };
        let mut pi = PROCESS_INFORMATION::default();
        unsafe {
            let _ = CreateProcessW(
                None,
                Some(PWSTR(cmd_wide.as_mut_ptr())),
                None,
                None,
                false,
                flags,
                None,
                None,
                &si,
                &mut pi,
            );
            let _ = windows::Win32::Foundation::CloseHandle(pi.hProcess);
            let _ = windows::Win32::Foundation::CloseHandle(pi.hThread);
        }
    }
}

fn message_box(text: &str, title: &str, flags: MESSAGEBOX_STYLE) {
    let text_w: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
    let title_w: Vec<u16> = title.encode_utf16().chain(std::iter::once(0)).collect();
    unsafe {
        let _ = MessageBoxW(
            None,
            PCWSTR(text_w.as_ptr()),
            PCWSTR(title_w.as_ptr()),
            flags,
        );
    }
}

fn build_tooltip(cfg: &CaptureConfig) -> String {
    let target = if let Some(ref p) = cfg.process {
        p.clone()
    } else if let Some(ref t) = cfg.title {
        format!("'{}'", t)
    } else {
        "?".to_string()
    };
    format!("shot — watching {}", target)
}

fn wide_string(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

fn copy_to_wide_buf(s: &str, buf: &mut [u16]) {
    let wide: Vec<u16> = s.encode_utf16().collect();
    let len = wide.len().min(buf.len() - 1);
    buf[..len].copy_from_slice(&wide[..len]);
    buf[len] = 0;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn win_f12() {
        let (mods, vk) = parse_hotkey("Win+F12").unwrap();
        assert_eq!(mods, MOD_WIN | MOD_NOREPEAT);
        assert_eq!(vk, 0x7B); // VK_F12
    }

    #[test]
    fn ctrl_shift_a() {
        let (mods, vk) = parse_hotkey("Ctrl+Shift+A").unwrap();
        assert_eq!(mods, MOD_CONTROL | MOD_SHIFT | MOD_NOREPEAT);
        assert_eq!(vk, 0x41); // VK_A
    }

    #[test]
    fn alt_printscreen() {
        let (mods, vk) = parse_hotkey("Alt+PrintScreen").unwrap();
        assert_eq!(mods, MOD_ALT | MOD_NOREPEAT);
        assert_eq!(vk, 0x2C); // VK_SNAPSHOT
    }

    #[test]
    fn case_insensitive() {
        let (m1, v1) = parse_hotkey("Win+F12").unwrap();
        let (m2, v2) = parse_hotkey("win+f12").unwrap();
        assert_eq!((m1, v1), (m2, v2));
    }

    #[test]
    fn ctrl_shift_f5() {
        let (mods, vk) = parse_hotkey("ctrl+shift+f5").unwrap();
        assert_eq!(mods, MOD_CONTROL | MOD_SHIFT | MOD_NOREPEAT);
        assert_eq!(vk, 0x74); // VK_F5
    }

    #[test]
    fn no_modifier_errors() {
        let err = parse_hotkey("F12").unwrap_err();
        assert!(err.to_string().contains("at least one modifier"));
    }

    #[test]
    fn no_key_errors() {
        let err = parse_hotkey("Win+").unwrap_err();
        assert!(err.to_string().contains("no key specified"));
    }

    #[test]
    fn two_keys_errors() {
        let err = parse_hotkey("Win+F12+F5").unwrap_err();
        assert!(err.to_string().contains("multiple keys"));
    }

    #[test]
    fn invalid_key_errors() {
        let err = parse_hotkey("Win+InvalidKey").unwrap_err();
        assert!(err.to_string().contains("Invalid hotkey key"));
    }

    #[test]
    fn empty_string_errors() {
        assert!(parse_hotkey("").is_err());
    }

    #[test]
    fn win_digit_0() {
        let (mods, vk) = parse_hotkey("Win+0").unwrap();
        assert_eq!(mods, MOD_WIN | MOD_NOREPEAT);
        assert_eq!(vk, 0x30); // VK_0
    }

    #[test]
    fn win_z() {
        let (mods, vk) = parse_hotkey("Win+Z").unwrap();
        assert_eq!(mods, MOD_WIN | MOD_NOREPEAT);
        assert_eq!(vk, 0x5A); // VK_Z
    }

    #[test]
    fn prtsc_alias() {
        let (mods, vk) = parse_hotkey("Alt+PrtSc").unwrap();
        assert_eq!(mods, MOD_ALT | MOD_NOREPEAT);
        assert_eq!(vk, 0x2C);
    }

    #[test]
    fn f1_and_f24_boundaries() {
        let (_, vk1) = parse_hotkey("Win+F1").unwrap();
        assert_eq!(vk1, 0x70); // VK_F1
        let (_, vk24) = parse_hotkey("Win+F24").unwrap();
        assert_eq!(vk24, 0x87); // VK_F24
    }

    #[test]
    fn control_alias() {
        let (mods, _) = parse_hotkey("Control+F1").unwrap();
        assert_eq!(mods, MOD_CONTROL | MOD_NOREPEAT);
    }

    #[test]
    fn quote_arg_simple() {
        assert_eq!(quote_arg("hello"), "\"hello\"");
    }

    #[test]
    fn quote_arg_trailing_backslash() {
        // C:\captures\ must not leave the closing quote escaped
        assert_eq!(quote_arg(r"C:\captures\"), r#""C:\captures\\""#);
    }

    #[test]
    fn quote_arg_embedded_quote() {
        assert_eq!(quote_arg(r#"say "hi""#), r#""say \"hi\"""#);
    }

    #[test]
    fn quote_arg_backslash_before_quote() {
        // backslash before embedded quote must be doubled
        assert_eq!(quote_arg(r#"a\"b"#), r#""a\\\"b""#);
    }

    #[test]
    fn temp_file_roundtrip_hotkey_error() {
        let pid: u32 = 9_999_999;
        write_temp_file(pid, "hotkey-error", "RegisterHotKey failed");
        let err = read_and_delete_temp_file(pid);
        assert!(matches!(err, ShotError::HotkeyError(_)));
        assert!(err.to_string().contains("RegisterHotKey failed"));
        assert!(!temp_file_path(pid).exists());
    }

    #[test]
    fn temp_file_watch_already_running() {
        let pid: u32 = 9_999_998;
        write_temp_file(pid, "watch-already-running", "daemon is already running");
        let err = read_and_delete_temp_file(pid);
        assert!(matches!(err, ShotError::WatchAlreadyRunning));
        assert!(!temp_file_path(pid).exists());
    }

    #[test]
    fn temp_file_tray_error() {
        let pid: u32 = 9_999_997;
        write_temp_file(pid, "tray-error", "Shell_NotifyIcon failed");
        let err = read_and_delete_temp_file(pid);
        assert!(matches!(err, ShotError::TrayError(_)));
        assert!(err.to_string().contains("Shell_NotifyIcon"));
        assert!(!temp_file_path(pid).exists());
    }

    #[test]
    fn temp_file_missing_falls_back() {
        let pid: u32 = 9_999_996;
        let _ = std::fs::remove_file(temp_file_path(pid));
        let err = read_and_delete_temp_file(pid);
        assert!(matches!(err, ShotError::HotkeyError(_)));
    }
}
