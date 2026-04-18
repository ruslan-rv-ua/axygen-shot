use crate::errors::ShotError;
use std::ffi::OsString;
use std::os::windows::ffi::OsStringExt;
use windows::Win32::Foundation::{CloseHandle, HWND, LPARAM, MAX_PATH, TRUE};
use windows::Win32::System::Threading::{
    OpenProcess, PROCESS_NAME_FORMAT, PROCESS_QUERY_LIMITED_INFORMATION, QueryFullProcessImageNameW,
};
use windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GetClassNameW, GetForegroundWindow, GetWindowTextLengthW, GetWindowTextW,
    GetWindowThreadProcessId, IsWindowVisible,
};
use windows::core::PWSTR;

#[derive(Clone, Debug)]
pub struct WindowInfo {
    pub hwnd: isize,
    pub title: String,
    pub process_name: String,
    pub pid: u32,
}

/// Injectable window enumeration — enables testing without Win32.
pub trait WindowEnumerator {
    fn enumerate(&self) -> Vec<WindowInfo>;
    fn get_foreground(&self) -> Option<isize>;
}

/// Resolve a single window matching the given criteria.
/// When both process and title provided: OR logic (match either).
/// When multiple matches: prefer foreground window, else first in z-order.
pub fn resolve(
    enumerator: &dyn WindowEnumerator,
    process: Option<&str>,
    title: Option<&str>,
) -> Result<WindowInfo, ShotError> {
    let all = enumerator.enumerate();
    let title_lower = title.map(|t| t.to_lowercase());
    let matches: Vec<&WindowInfo> = all
        .iter()
        .filter(|w| {
            // Process: exact match (user types the .exe name)
            let process_match = process
                .map(|p| w.process_name.eq_ignore_ascii_case(p))
                .unwrap_or(false);
            // Title: substring match (user types any fragment of the window title)
            let title_match = title_lower
                .as_ref()
                .map(|t| w.title.to_lowercase().contains(t.as_str()))
                .unwrap_or(false);
            process_match || title_match
        })
        .collect();

    if matches.is_empty() {
        let target = format!(
            "process={}, title={}",
            process.unwrap_or("(none)"),
            title.unwrap_or("(none)"),
        );
        return Err(ShotError::WindowNotFound(target));
    }

    let foreground = enumerator.get_foreground();
    if let Some(fg_hwnd) = foreground
        && let Some(fg_match) = matches.iter().find(|w| w.hwnd == fg_hwnd)
    {
        return Ok((*fg_match).clone());
    }

    Ok(matches[0].clone())
}

/// List all visible top-level windows.
/// Excludes empty titles. Shell windows are excluded by the enumerator.
pub fn list_all(enumerator: &dyn WindowEnumerator) -> Vec<WindowInfo> {
    enumerator
        .enumerate()
        .into_iter()
        .filter(|w| !w.title.is_empty())
        .collect()
}

/// Live Win32 window enumerator using `EnumWindows` and related APIs.
pub struct Win32Enumerator;

impl WindowEnumerator for Win32Enumerator {
    fn enumerate(&self) -> Vec<WindowInfo> {
        let mut windows: Vec<WindowInfo> = Vec::new();
        unsafe {
            let _ = EnumWindows(Some(enum_callback), LPARAM(&raw mut windows as isize));
        }
        windows
    }

    fn get_foreground(&self) -> Option<isize> {
        unsafe {
            let hwnd = GetForegroundWindow();
            if hwnd.0.is_null() {
                None
            } else {
                Some(hwnd.0 as isize)
            }
        }
    }
}

// Safety: Panic here aborts (Edition 2024 extern "system" semantics).
// This is acceptable — OOM during enumeration is unrecoverable.
unsafe extern "system" fn enum_callback(hwnd: HWND, lparam: LPARAM) -> windows::core::BOOL {
    let windows = unsafe { &mut *(lparam.0 as *mut Vec<WindowInfo>) };

    if !unsafe { IsWindowVisible(hwnd) }.as_bool() {
        return TRUE;
    }

    let title_len = unsafe { GetWindowTextLengthW(hwnd) };
    if title_len <= 0 {
        return TRUE;
    }
    let mut title_buf = vec![0u16; (title_len + 1) as usize];
    unsafe { GetWindowTextW(hwnd, &mut title_buf) };
    let title = OsString::from_wide(&title_buf[..title_len as usize])
        .to_string_lossy()
        .to_string();

    if title.is_empty() {
        return TRUE;
    }

    let mut class_buf = [0u16; 256];
    let class_len = unsafe { GetClassNameW(hwnd, &mut class_buf) };
    if class_len > 0 {
        let class_name = OsString::from_wide(&class_buf[..class_len as usize])
            .to_string_lossy()
            .to_string();
        if class_name == "Shell_TrayWnd" || class_name == "Progman" || class_name == "WorkerW" {
            return TRUE;
        }
    }

    let mut pid: u32 = 0;
    unsafe { GetWindowThreadProcessId(hwnd, Some(&mut pid)) };

    let process_name = get_process_name(pid).unwrap_or_default();

    windows.push(WindowInfo {
        hwnd: hwnd.0 as isize,
        title,
        process_name,
        pid,
    });

    TRUE
}

fn get_process_name(pid: u32) -> Option<String> {
    unsafe {
        let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;
        let mut buf = [0u16; MAX_PATH as usize];
        let mut size = buf.len() as u32;
        let result = QueryFullProcessImageNameW(
            handle,
            PROCESS_NAME_FORMAT(0),
            PWSTR(buf.as_mut_ptr()),
            &mut size,
        );
        let _ = CloseHandle(handle);
        result.ok()?;
        let path = OsString::from_wide(&buf[..size as usize])
            .to_string_lossy()
            .to_string();
        path.rsplit('\\').next().map(ToString::to_string)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MockEnumerator {
        windows: Vec<WindowInfo>,
        foreground_hwnd: Option<isize>,
    }

    impl WindowEnumerator for MockEnumerator {
        fn enumerate(&self) -> Vec<WindowInfo> {
            self.windows.clone()
        }
        fn get_foreground(&self) -> Option<isize> {
            self.foreground_hwnd
        }
    }

    fn make_window(hwnd: isize, title: &str, process: &str, pid: u32) -> WindowInfo {
        WindowInfo {
            hwnd,
            title: title.to_string(),
            process_name: process.to_string(),
            pid,
        }
    }

    #[test]
    fn resolve_by_process_name() {
        let mock = MockEnumerator {
            windows: vec![make_window(1, "Title", "notepad.exe", 100)],
            foreground_hwnd: None,
        };
        let result = resolve(&mock, Some("notepad.exe"), None).unwrap();
        assert_eq!(result.hwnd, 1);
        assert_eq!(result.process_name, "notepad.exe");
    }

    #[test]
    fn resolve_by_title_substring() {
        let mock = MockEnumerator {
            windows: vec![make_window(2, "My Document - Notepad", "notepad.exe", 101)],
            foreground_hwnd: None,
        };
        let result = resolve(&mock, None, Some("Document")).unwrap();
        assert_eq!(result.hwnd, 2);
    }

    #[test]
    fn title_match_is_case_insensitive() {
        let mock = MockEnumerator {
            windows: vec![make_window(3, "UPPERCASE TITLE", "app.exe", 102)],
            foreground_hwnd: None,
        };
        let result = resolve(&mock, None, Some("uppercase")).unwrap();
        assert_eq!(result.hwnd, 3);
    }

    #[test]
    fn title_match_works_mid_title() {
        let mock = MockEnumerator {
            windows: vec![make_window(4, "Foo — MyApp — Bar", "app.exe", 103)],
            foreground_hwnd: None,
        };
        let result = resolve(&mock, None, Some("MyApp")).unwrap();
        assert_eq!(result.hwnd, 4);
    }

    #[test]
    fn both_process_and_title_uses_or_logic() {
        let mock = MockEnumerator {
            windows: vec![
                make_window(5, "Unrelated", "notepad.exe", 200),
                make_window(6, "Target Title", "other.exe", 201),
            ],
            foreground_hwnd: None,
        };
        // Process match → finds window 5
        let r1 = resolve(&mock, Some("notepad.exe"), Some("Target")).unwrap();
        assert!(r1.hwnd == 5 || r1.hwnd == 6);
    }

    #[test]
    fn no_match_returns_error() {
        let mock = MockEnumerator {
            windows: vec![make_window(7, "Other", "other.exe", 300)],
            foreground_hwnd: None,
        };
        assert!(resolve(&mock, Some("missing.exe"), None).is_err());
    }

    #[test]
    fn multiple_matches_prefers_foreground() {
        let mock = MockEnumerator {
            windows: vec![
                make_window(8, "Title A", "app.exe", 400),
                make_window(9, "Title B", "app.exe", 401),
            ],
            foreground_hwnd: Some(9),
        };
        let result = resolve(&mock, Some("app.exe"), None).unwrap();
        assert_eq!(result.hwnd, 9);
    }

    #[test]
    fn multiple_matches_no_foreground_returns_first() {
        let mock = MockEnumerator {
            windows: vec![
                make_window(10, "Title A", "app.exe", 500),
                make_window(11, "Title B", "app.exe", 501),
            ],
            foreground_hwnd: Some(99), // Not among matches
        };
        let result = resolve(&mock, Some("app.exe"), None).unwrap();
        assert_eq!(result.hwnd, 10);
    }
}
