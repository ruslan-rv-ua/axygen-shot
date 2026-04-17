use crate::config::CaptureConfig;
use crate::errors::ShotError;

use std::ffi::OsString;
use std::os::windows::ffi::OsStrExt;
use windows::Win32::System::Console::GetConsoleWindow;
use windows::Win32::System::Threading::{
    CreateProcessW, PROCESS_CREATION_FLAGS, PROCESS_INFORMATION, STARTUPINFOW,
};
use windows::core::PWSTR;

const CREATE_NEW_PROCESS_GROUP: u32 = 0x00000200;
const DETACHED_PROCESS: u32 = 0x00000008;
const CREATE_NO_WINDOW: u32 = 0x08000000;

// Win32 hotkey modifier flags
const MOD_ALT: u32 = 0x0001;
const MOD_CONTROL: u32 = 0x0002;
const MOD_SHIFT: u32 = 0x0004;
const MOD_WIN: u32 = 0x0008;
const MOD_NOREPEAT: u32 = 0x4000;

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

fn launch_daemon() -> Result<u32, ShotError> {
    let exe = std::env::current_exe()
        .map_err(|e| ShotError::HotkeyError(format!("Cannot find own executable: {}", e)))?;

    let mut cmd_line = OsString::new();
    cmd_line.push("\"");
    cmd_line.push(exe.as_os_str());
    cmd_line.push("\"");
    for arg in std::env::args().skip(1) {
        cmd_line.push(" \"");
        cmd_line.push(arg.replace('"', "\\\"").as_str());
        cmd_line.push("\"");
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

fn run_daemon(_cfg: &CaptureConfig) -> Result<(), ShotError> {
    Ok(())
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
}
