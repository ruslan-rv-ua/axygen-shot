use crate::config::CaptureConfig;
use crate::errors::ShotError;
use crate::{audio, capture, clipboard, errors, storage, window_resolver};
use crate::window_resolver::Win32Enumerator;

use std::ffi::OsString;
use std::mem;
use std::os::windows::ffi::OsStrExt;
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, WPARAM};
use windows::Win32::System::Console::GetConsoleWindow;
use windows::Win32::System::Threading::{
    CreateProcessW, PROCESS_CREATION_FLAGS, PROCESS_INFORMATION, STARTUPINFOW,
};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    HOT_KEY_MODIFIERS, RegisterHotKey, UnregisterHotKey,
};
use windows::Win32::UI::Shell::{
    NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE, NOTIFYICONDATAW, Shell_NotifyIconW,
};
use windows::Win32::UI::WindowsAndMessaging::{
    AppendMenuW, CreatePopupMenu, CreateWindowExW, DefWindowProcW, DestroyMenu, DispatchMessageW,
    GetCursorPos, GetMessageW, HWND_MESSAGE, IDI_APPLICATION, LoadIconW, MB_ICONERROR,
    MENU_ITEM_FLAGS, MESSAGEBOX_STYLE, MSG, MessageBoxW, PostMessageW, PostQuitMessage,
    RegisterClassExW, SetForegroundWindow, TPM_RIGHTALIGN, TrackPopupMenu, TranslateMessage,
    WINDOW_EX_STYLE, WINDOW_STYLE, WNDCLASSEXW, WM_APP, WM_COMMAND, WM_HOTKEY, WM_NULL,
    WM_RBUTTONUP,
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

static mut DAEMON_CFG: Option<*const CaptureConfig> = None;
static mut CAPTURING: bool = false;

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
                    return Err(ShotError::ArgError(
                        format!("Invalid hotkey '{}': multiple keys specified", s),
                    ));
                }
                vk = Some(parse_vk(key, s)?);
            }
        }
    }

    let vk = vk.ok_or_else(|| {
        ShotError::ArgError(format!("Invalid hotkey '{}': no key specified", s))
    })?;

    if modifiers == 0 {
        return Err(ShotError::ArgError(format!(
            "Invalid hotkey '{}': at least one modifier required (Win, Ctrl, Shift, Alt)",
            s,
        )));
    }

    Ok((modifiers | MOD_NOREPEAT, vk))
}

fn parse_vk(key: &str, full_hotkey: &str) -> Result<u32, ShotError> {
    let lower = key.to_lowercase();

    // F1–F24
    if lower.starts_with('f') {
        if let Ok(n) = lower[1..].parse::<u32>() {
            if (1..=24).contains(&n) {
                return Ok(0x6F + n); // VK_F1 = 0x70
            }
        }
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
        _ => Err(ShotError::ArgError(format!(
            "Invalid hotkey key '{}' in '{}' — valid keys: F1-F24, A-Z, 0-9, PrintScreen",
            key, full_hotkey,
        ))),
    }
}

pub fn run(cfg: &CaptureConfig) -> Result<(), ShotError> {
    if !is_detached() {
        let child_pid = launch_daemon()?;
        if !cfg.quiet || cfg.verbose {
            println!("status: ok\nwatch: started (PID {})", child_pid);
        }
        return Ok(());
    }
    run_daemon(cfg)
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

fn launch_daemon() -> Result<u32, ShotError> {
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
    let mut cmd_wide: Vec<u16> = cmd_line.encode_wide().chain(std::iter::once(0)).collect();

    let flags = PROCESS_CREATION_FLAGS(CREATE_NEW_PROCESS_GROUP | DETACHED_PROCESS | CREATE_NO_WINDOW);
    let mut si = STARTUPINFOW::default();
    si.cb = std::mem::size_of::<STARTUPINFOW>() as u32;
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

fn run_daemon(cfg: &CaptureConfig) -> Result<(), ShotError> {
    // Parse hotkey before creating any windows
    let (modifiers, vk) = parse_hotkey(&cfg.hotkey)?;

    // Store config pointer for wndproc access (single-threaded daemon, safe)
    unsafe { DAEMON_CFG = Some(cfg as *const CaptureConfig) };

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
            0, 0, 0, 0,
            Some(HWND_MESSAGE),
            None,
            None,
            None,
        )
    };
    let hwnd = match hwnd {
        Ok(h) if !h.0.is_null() => h,
        _ => return Err(ShotError::HotkeyError("Cannot create message window".into())),
    };

    // Register global hotkey
    let hotkey_result = unsafe {
        RegisterHotKey(Some(hwnd), HOTKEY_ID, HOT_KEY_MODIFIERS(modifiers), vk)
    };
    if let Err(e) = hotkey_result {
        message_box(
            &format!(
                "Cannot register hotkey '{}': {}\n\nThe hotkey may be in use by another application.",
                cfg.hotkey, e
            ),
            "Axygen Shot — Error",
            MB_ICONERROR,
        );
        return Err(ShotError::HotkeyError(format!(
            "RegisterHotKey failed for '{}': {}",
            cfg.hotkey, e
        )));
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
        unsafe { let _ = UnregisterHotKey(Some(hwnd), HOTKEY_ID); }
        message_box(
            "Cannot create tray icon. The system tray may not be available.",
            "Axygen Shot — Error",
            MB_ICONERROR,
        );
        return Err(ShotError::HotkeyError("Shell_NotifyIcon failed".into()));
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
    }

    Ok(())
}

unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        WM_HOTKEY => {
            if unsafe { CAPTURING } {
                audio::play_busy();
            } else {
                unsafe { CAPTURING = true };
                if let Some(cfg_ptr) = unsafe { DAEMON_CFG } {
                    let cfg = unsafe { &*cfg_ptr };
                    do_capture(cfg);
                }
                unsafe { CAPTURING = false };
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
            let id = (wparam.0 & 0xFFFF) as usize;
            match id {
                ID_EXIT => {
                    unsafe { PostQuitMessage(0) };
                }
                ID_RESTART => {
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
        let window = window_resolver::resolve(
            &enumerator,
            cfg.process.as_deref(),
            cfg.title.as_deref(),
        )?;
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
            &capture_result.png_bytes,
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
            cmd_line.push(" ");
            cmd_line.push(quote_arg(&arg).as_str());
        }
        let mut cmd_wide: Vec<u16> = cmd_line.encode_wide().chain(std::iter::once(0)).collect();
        let flags = PROCESS_CREATION_FLAGS(CREATE_NEW_PROCESS_GROUP | DETACHED_PROCESS | CREATE_NO_WINDOW);
        let mut si = STARTUPINFOW::default();
        si.cb = mem::size_of::<STARTUPINFOW>() as u32;
        let mut pi = PROCESS_INFORMATION::default();
        unsafe {
            let _ = CreateProcessW(
                None,
                Some(PWSTR(cmd_wide.as_mut_ptr())),
                None, None, false, flags, None, None, &si, &mut pi,
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
}
