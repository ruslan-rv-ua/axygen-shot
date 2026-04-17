unsafe extern "system" {
    fn MessageBeep(uType: u32) -> i32;
}

const MB_ICONASTERISK: u32 = 0x00000040;
const MB_ICONHAND: u32 = 0x00000010;

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
