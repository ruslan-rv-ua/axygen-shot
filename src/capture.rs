use crate::errors::ShotError;
use std::mem;
use windows::Win32::Foundation::HWND;
use windows::Win32::Graphics::Gdi::{
    BI_RGB, BITMAPINFO, BITMAPINFOHEADER, BitBlt, CreateCompatibleBitmap, CreateCompatibleDC,
    DIB_RGB_COLORS, DeleteDC, DeleteObject, GetDIBits, GetWindowDC, ReleaseDC, SRCCOPY,
    SelectObject,
};
use windows::Win32::Storage::Xps::{PRINT_WINDOW_FLAGS, PrintWindow};
use windows::Win32::UI::WindowsAndMessaging::{GetWindowRect, IsIconic};

/// PW_RENDERFULLCONTENT — undocumented flag for full window capture.
const PW_RENDERFULLCONTENT: PRINT_WINDOW_FLAGS = PRINT_WINDOW_FLAGS(0x00000002);

pub struct CaptureResult {
    pub png_bytes: Vec<u8>,
    pub width: u32,
    pub height: u32,
}

/// Capture a window as PNG bytes.
/// Returns WindowMinimized if target is iconic.
/// Returns CaptureFailed if PrintWindow or encoding fails.
pub fn capture_window(hwnd: isize) -> Result<CaptureResult, ShotError> {
    unsafe {
        let hwnd = HWND(hwnd as *mut _);

        if IsIconic(hwnd).as_bool() {
            return Err(ShotError::WindowMinimized);
        }

        let mut rect = mem::zeroed();
        GetWindowRect(hwnd, &mut rect)
            .map_err(|e| ShotError::CaptureFailed(format!("GetWindowRect failed: {}", e)))?;

        let width = (rect.right - rect.left) as u32;
        let height = (rect.bottom - rect.top) as u32;

        if width == 0 || height == 0 {
            return Err(ShotError::CaptureFailed(
                "Window has zero dimensions".into(),
            ));
        }

        let screen_dc = GetWindowDC(Some(hwnd));
        let mem_dc = CreateCompatibleDC(Some(screen_dc));
        let bitmap = CreateCompatibleBitmap(screen_dc, width as i32, height as i32);
        let old_obj = SelectObject(mem_dc, bitmap.into());

        let print_result = PrintWindow(hwnd, mem_dc, PW_RENDERFULLCONTENT);
        if !print_result.as_bool() {
            let blt_result = BitBlt(
                mem_dc,
                0,
                0,
                width as i32,
                height as i32,
                Some(screen_dc),
                0,
                0,
                SRCCOPY,
            );
            if blt_result.is_err() {
                SelectObject(mem_dc, old_obj);
                let _ = DeleteObject(bitmap.into());
                let _ = DeleteDC(mem_dc);
                ReleaseDC(Some(hwnd), screen_dc);
                return Err(ShotError::CaptureFailed(
                    "PrintWindow and BitBlt both failed. The window may require elevated \
                     privileges or use GPU-accelerated rendering."
                        .into(),
                ));
            }
        }

        let mut bmi = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: width as i32,
                biHeight: -(height as i32), // top-down DIB
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..mem::zeroed()
            },
            ..mem::zeroed()
        };

        let mut pixels = vec![0u8; (width * height * 4) as usize];
        let lines = GetDIBits(
            mem_dc,
            bitmap,
            0,
            height,
            Some(pixels.as_mut_ptr().cast()),
            &mut bmi,
            DIB_RGB_COLORS,
        );

        SelectObject(mem_dc, old_obj);
        let _ = DeleteObject(bitmap.into());
        let _ = DeleteDC(mem_dc);
        ReleaseDC(Some(hwnd), screen_dc);

        if lines == 0 {
            return Err(ShotError::CaptureFailed(
                "GetDIBits returned 0 lines".into(),
            ));
        }

        // Convert BGRA → RGBA
        for chunk in pixels.chunks_exact_mut(4) {
            chunk.swap(0, 2);
        }

        let mut png_buf = Vec::new();
        {
            let mut encoder = png::Encoder::new(&mut png_buf, width, height);
            encoder.set_color(png::ColorType::Rgba);
            encoder.set_depth(png::BitDepth::Eight);
            let mut writer = encoder
                .write_header()
                .map_err(|e| ShotError::CaptureFailed(format!("PNG header error: {}", e)))?;
            writer
                .write_image_data(&pixels)
                .map_err(|e| ShotError::CaptureFailed(format!("PNG encode error: {}", e)))?;
        }

        Ok(CaptureResult {
            png_bytes: png_buf,
            width,
            height,
        })
    }
}
