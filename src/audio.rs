#[link(name = "user32")]
unsafe extern "system" {
    fn MessageBeep(uType: u32) -> i32;
}

const MB_ICONASTERISK: u32 = 0x00000040;
const MB_ICONHAND: u32 = 0x00000010;
const MB_ICONEXCLAMATION: u32 = 0x00000030;
const MB_ICONQUESTION: u32 = 0x00000020;

/// Play success sound (SystemAsterisk) — PRD story 57.
/// Fire-and-forget: errors silently ignored.
pub fn play_success() {
    unsafe {
        let _ = MessageBeep(MB_ICONASTERISK);
    }
}

/// Play error sound (SystemHand) — PRD story 58.
/// Fire-and-forget: errors silently ignored.
pub fn play_error() {
    unsafe {
        let _ = MessageBeep(MB_ICONHAND);
    }
}

/// Play startup sound (SystemExclamation) — PRD story 59.
/// Used when watch mode successfully registers its hotkey.
/// Fire-and-forget: errors silently ignored.
pub fn play_startup() {
    unsafe {
        let _ = MessageBeep(MB_ICONEXCLAMATION);
    }
}

/// Play busy sound (SystemQuestion) — spec: capture serialization.
/// Used when hotkey fires during an active capture.
/// Fire-and-forget: errors silently ignored.
pub fn play_busy() {
    unsafe {
        let _ = MessageBeep(MB_ICONQUESTION);
    }
}
