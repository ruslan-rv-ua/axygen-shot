use crate::config::ClipboardMode;
use crate::errors::ShotError;
use std::mem;
use std::path::Path;
use windows::Win32::Foundation::HANDLE;
use windows::Win32::Graphics::Gdi::BITMAPINFOHEADER;
use windows::Win32::System::DataExchange::{
    CloseClipboard, EmptyClipboard, OpenClipboard, SetClipboardData,
};
use windows::Win32::System::Memory::{GMEM_MOVEABLE, GlobalAlloc, GlobalLock, GlobalUnlock};

#[link(name = "kernel32")]
unsafe extern "system" {
    fn GlobalFree(hmem: *mut std::ffi::c_void) -> *mut std::ffi::c_void;
}
use windows::Win32::System::Ole::{CF_DIB, CF_UNICODETEXT};

/// Write to clipboard based on mode.
pub fn write_clipboard(
    mode: ClipboardMode,
    file_path: &Path,
    bgra_pixels: &[u8],
    width: u32,
    height: u32,
) -> Result<(), ShotError> {
    unsafe {
        OpenClipboard(None)
            .map_err(|e| ShotError::ClipboardError(format!("OpenClipboard failed: {}", e)))?;
        let _ = EmptyClipboard();

        let result = match mode {
            ClipboardMode::Path => set_path(file_path),
            ClipboardMode::Image => set_image(bgra_pixels, width, height),
            ClipboardMode::Both => {
                set_path(file_path).and_then(|()| set_image(bgra_pixels, width, height))
            }
        };

        let _ = CloseClipboard();
        result
    }
}

/// Set CF_UNICODETEXT with absolute file path.
unsafe fn set_path(file_path: &Path) -> Result<(), ShotError> {
    let path_str = file_path
        .to_str()
        .ok_or_else(|| ShotError::ClipboardError("Path is not valid UTF-8".into()))?;
    let wide: Vec<u16> = path_str.encode_utf16().chain(std::iter::once(0)).collect();
    let byte_len = wide.len() * mem::size_of::<u16>();

    let hmem = unsafe { GlobalAlloc(GMEM_MOVEABLE, byte_len) }
        .map_err(|e| ShotError::ClipboardError(format!("GlobalAlloc failed: {}", e)))?;
    let ptr = unsafe { GlobalLock(hmem) };
    if ptr.is_null() {
        let _ = unsafe { GlobalFree(hmem.0) };
        return Err(ShotError::ClipboardError("GlobalLock returned null".into()));
    }
    unsafe {
        std::ptr::copy_nonoverlapping(wide.as_ptr() as *const u8, ptr as *mut u8, byte_len);
        let _ = GlobalUnlock(hmem);
    }
    if let Err(e) = unsafe { SetClipboardData(CF_UNICODETEXT.0 as u32, Some(HANDLE(hmem.0))) } {
        let _ = unsafe { GlobalFree(hmem.0) };
        return Err(ShotError::ClipboardError(format!(
            "SetClipboardData(text) failed: {}",
            e
        )));
    }

    Ok(())
}

/// Set CF_DIB with device-independent bitmap.
/// Accepts raw BGRA pixels in top-down order and flips to bottom-up for DIB.
unsafe fn set_image(bgra_topdown: &[u8], width: u32, height: u32) -> Result<(), ShotError> {
    let pixel_count = (width as usize)
        .checked_mul(height as usize)
        .and_then(|n| n.checked_mul(4))
        .ok_or_else(|| ShotError::ClipboardError("Image too large for clipboard".into()))?;
    let stride = width as usize * 4;

    if bgra_topdown.len() < pixel_count {
        return Err(ShotError::ClipboardError("BGRA buffer too small".into()));
    }

    // Flip vertically: top-down → bottom-up for DIB
    let mut bgra_bottomup = vec![0u8; pixel_count];
    for y in 0..height as usize {
        let src_row = &bgra_topdown[y * stride..(y + 1) * stride];
        let dst_row_start = (height as usize - 1 - y) * stride;
        bgra_bottomup[dst_row_start..dst_row_start + stride].copy_from_slice(src_row);
    }

    // Build DIB: header + pixels
    let header_size = mem::size_of::<BITMAPINFOHEADER>();
    let pixel_size = bgra_bottomup.len();
    let total_size = header_size + pixel_size;

    let header = BITMAPINFOHEADER {
        biSize: header_size as u32,
        biWidth: width as i32,
        biHeight: height as i32, // Positive = bottom-up
        biPlanes: 1,
        biBitCount: 32,
        biCompression: 0, // BI_RGB
        biSizeImage: pixel_size as u32,
        ..unsafe { mem::zeroed() }
    };

    let hmem = unsafe { GlobalAlloc(GMEM_MOVEABLE, total_size) }
        .map_err(|e| ShotError::ClipboardError(format!("GlobalAlloc failed: {}", e)))?;
    let ptr = unsafe { GlobalLock(hmem) };
    if ptr.is_null() {
        let _ = unsafe { GlobalFree(hmem.0) };
        return Err(ShotError::ClipboardError("GlobalLock returned null".into()));
    }
    unsafe {
        std::ptr::copy_nonoverlapping(
            &header as *const BITMAPINFOHEADER as *const u8,
            ptr as *mut u8,
            header_size,
        );
        std::ptr::copy_nonoverlapping(
            bgra_bottomup.as_ptr(),
            (ptr as *mut u8).add(header_size),
            pixel_size,
        );
        let _ = GlobalUnlock(hmem);
    }
    if let Err(e) = unsafe { SetClipboardData(CF_DIB.0 as u32, Some(HANDLE(hmem.0))) } {
        let _ = unsafe { GlobalFree(hmem.0) };
        return Err(ShotError::ClipboardError(format!(
            "SetClipboardData(DIB) failed: {}",
            e
        )));
    }

    Ok(())
}
