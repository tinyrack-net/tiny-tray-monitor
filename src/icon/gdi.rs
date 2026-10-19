//! GDI 텍스트 래스터라이저.
//!
//! `unsafe`는 이 모듈에만 둔다. 바깥에는 흑백 커버리지 버퍼만 넘긴다.
//!
//! GDI 리소스는 [Resources]의 `Drop`에서 정리 순서를 고정해 누수를 막는다.

use core::ffi::c_void;
use core::mem::size_of;

use windows::Win32::Foundation::{COLORREF, RECT};
use windows::Win32::Graphics::Gdi::{
    BI_RGB, BITMAPINFO, BITMAPINFOHEADER, CreateCompatibleDC, CreateDIBSection, CreateFontW,
    DIB_RGB_COLORS, DT_LEFT, DT_NOCLIP, DT_SINGLELINE, DeleteDC, DeleteObject, DrawTextW,
    FONT_CHARSET, FONT_CLIP_PRECISION, FONT_OUTPUT_PRECISION, FONT_QUALITY, GetDC,
    GetTextExtentPoint32W, HBITMAP, HDC, HFONT, HGDIOBJ, ReleaseDC, SelectObject, SetBkColor,
    SetTextColor,
};

/// 획 굵기. 작은 크기에서 획이 살아남도록 semi-bold를 쓴다.
const FONT_WEIGHT: i32 = 600;

/// 흑백 커버리지 버퍼. 0은 배경, 255는 완전한 잉크.
#[derive(Clone, PartialEq, Eq)]
pub struct Coverage {
    pub width: i32,
    pub height: i32,
    pub data: Vec<u8>,
}

impl Coverage {
    pub fn new(width: i32, height: i32, data: Vec<u8>) -> Self {
        debug_assert_eq!(data.len(), (width * height) as usize);
        Self {
            width,
            height,
            data,
        }
    }

    /// 범위를 벗어나면 0을 돌려준다.
    pub fn get(&self, x: i32, y: i32) -> u8 {
        if x < 0 || y < 0 || x >= self.width || y >= self.height {
            return 0;
        }
        self.data[(y * self.width + x) as usize]
    }
}

impl core::fmt::Debug for Coverage {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Coverage")
            .field("width", &self.width)
            .field("height", &self.height)
            .finish()
    }
}

/// GDI 리소스 묶음. `Drop`에서 정리 순서를 보장한다.
struct Resources {
    screen: HDC,
    dc: HDC,
    bitmap: HBITMAP,
    font: HFONT,
    old_bitmap: HGDIOBJ,
    old_font: HGDIOBJ,
}

impl Drop for Resources {
    fn drop(&mut self) {
        unsafe {
            SelectObject(self.dc, self.old_bitmap);
            SelectObject(self.dc, self.old_font);
            let _ = DeleteObject(HGDIOBJ(self.bitmap.0));
            let _ = DeleteObject(HGDIOBJ(self.font.0));
            let _ = DeleteDC(self.dc);
            ReleaseDC(None, self.screen);
        }
    }
}

/// `text`를 `font_px` 높이로 그려 커버리지를 돌려준다.
///
/// 빈 문자열은 GDI에 dangling 포인터가 넘어가 프로세스가 죽으므로(실측)
/// 호출 자체를 막는다.
pub fn rasterize(text: &str, font_px: i32) -> Option<Coverage> {
    let mut wide: Vec<u16> = text.encode_utf16().collect();
    if wide.is_empty() || font_px < 1 {
        return None;
    }
    unsafe { rasterize_utf16(&mut wide, font_px) }
}

unsafe fn rasterize_utf16(wide: &mut [u16], font_px: i32) -> Option<Coverage> {
    let screen = unsafe { GetDC(None) };
    if screen.is_invalid() {
        return None;
    }

    let dc = unsafe { CreateCompatibleDC(Some(screen)) };
    if dc.is_invalid() {
        unsafe { ReleaseDC(None, screen) };
        return None;
    }

    let font = unsafe {
        CreateFontW(
            -font_px,
            0,
            0,
            0,
            FONT_WEIGHT,
            0, // italic
            0, // underline
            0, // strikeout
            FONT_CHARSET(0),
            FONT_OUTPUT_PRECISION(0),
            FONT_CLIP_PRECISION(0),
            FONT_QUALITY(0),
            0, // pitch and family
            windows::core::w!("Segoe UI"),
        )
    };
    let old_font = unsafe { SelectObject(dc, HGDIOBJ(font.0)) };

    let mut measured = windows::Win32::Foundation::SIZE::default();
    let _ = unsafe { GetTextExtentPoint32W(dc, wide, &mut measured) };

    // 글자 주변에 여유를 둬야 첫/마지막 획이 잘리지 않는다.
    let pad = font_px / 4 + 2;
    let canvas_w = (measured.cx + pad * 2).max(font_px);
    let canvas_h = (measured.cy + pad * 2).max(font_px);

    let bmi = BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: canvas_w,
            biHeight: -canvas_h, // top-down
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB.0,
            ..Default::default()
        },
        ..Default::default()
    };
    let mut bits: *mut c_void = std::ptr::null_mut();
    let bitmap =
        match unsafe { CreateDIBSection(Some(dc), &bmi, DIB_RGB_COLORS, &mut bits, None, 0) } {
            Ok(bitmap) if !bits.is_null() => bitmap,
            _ => {
                let _ = unsafe { DeleteObject(HGDIOBJ(font.0)) };
                let _ = unsafe { DeleteDC(dc) };
                unsafe { ReleaseDC(None, screen) };
                return None;
            }
        };
    let old_bitmap = unsafe { SelectObject(dc, HGDIOBJ(bitmap.0)) };

    // 흰 글자 / 검은 배경으로 그린 뒤 휘도를 커버리지로 쓴다.
    unsafe {
        SetBkColor(dc, COLORREF(0x0000_0000));
        SetTextColor(dc, COLORREF(0x00FF_FFFF));
    }
    let mut rect = RECT {
        left: 0,
        top: 0,
        right: canvas_w,
        bottom: canvas_h,
    };
    unsafe { DrawTextW(dc, wide, &mut rect, DT_LEFT | DT_NOCLIP | DT_SINGLELINE) };

    let rgba = unsafe {
        std::slice::from_raw_parts(bits as *const u8, (canvas_w * canvas_h * 4) as usize)
    };
    // BI_RGB 32bpp DIB에서 GDI는 B, G, R만 채우고 예약 바이트는 0으로 둔다.
    // 흰 글자를 그렸으므로 R 채널이 곧 커버리지다.
    let data = (0..(canvas_w * canvas_h) as usize)
        .map(|i| rgba[i * 4 + 2])
        .collect();

    let resources = Resources {
        screen,
        dc,
        bitmap,
        font,
        old_bitmap,
        old_font,
    };
    drop(resources);

    Some(Coverage::new(canvas_w, canvas_h, data))
}

/// RGBA 픽셀을 32bpp DIB 섹션으로 만들어 `HBITMAP`으로 돌려준다.
///
/// 정적 컨트롤(`SS_BITMAP`)은 알파를 합성하지 않고 비트를 그대로 복사하므로, 호출자는
/// 배경과 **이미 합성한** 불투명 픽셀을 넘겨야 한다.
pub fn bitmap_from_rgba(width: i32, height: i32, rgba: &[u8]) -> Option<HBITMAP> {
    if width <= 0 || height <= 0 || rgba.len() != (width * height * 4) as usize {
        return None;
    }

    let screen = unsafe { GetDC(None) };
    if screen.is_invalid() {
        return None;
    }

    let info = BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: width,
            biHeight: -height, // top-down
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB.0,
            ..Default::default()
        },
        ..Default::default()
    };
    let mut bits: *mut c_void = std::ptr::null_mut();
    let created =
        unsafe { CreateDIBSection(Some(screen), &info, DIB_RGB_COLORS, &mut bits, None, 0) };
    unsafe { ReleaseDC(None, screen) };

    let bitmap = match created {
        Ok(bitmap) if !bits.is_null() => bitmap,
        Ok(bitmap) => {
            let _ = unsafe { DeleteObject(HGDIOBJ(bitmap.0)) };
            return None;
        }
        Err(_) => return None,
    };

    // GDI는 B, G, R만 읽고 예약 바이트는 무시한다.
    let pixels =
        unsafe { std::slice::from_raw_parts_mut(bits as *mut u8, (width * height * 4) as usize) };
    for (destination, source) in pixels.chunks_exact_mut(4).zip(rgba.chunks_exact(4)) {
        destination.copy_from_slice(&[source[2], source[1], source[0], 255]);
    }

    Some(bitmap)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_text_is_rejected() {
        assert!(rasterize("", 64).is_none());
    }

    #[test]
    fn non_positive_font_is_rejected() {
        assert!(rasterize("80%", 0).is_none());
        assert!(rasterize("80%", -3).is_none());
    }

    #[test]
    fn text_produces_ink() {
        let cov = rasterize("80%", 128).expect("rasterize");
        assert!(cov.width > 0 && cov.height > 0);
        assert_eq!(cov.data.len(), (cov.width * cov.height) as usize);
        let ink = cov.data.iter().filter(|&&v| v > 8).count();
        assert!(ink > 0, "잉크 픽셀이 있어야 한다");
        assert!(ink < cov.data.len(), "배경도 남아 있어야 한다");
    }

    #[test]
    fn longer_text_is_wider() {
        let short = rasterize("8", 128).expect("rasterize");
        let long = rasterize("8888", 128).expect("rasterize");
        assert!(long.width > short.width);
    }

    #[test]
    fn rasterize_is_deterministic() {
        let a = rasterize("CPU", 96).expect("rasterize");
        let b = rasterize("CPU", 96).expect("rasterize");
        assert_eq!(a, b, "같은 입력은 같은 결과를 내야 한다");
    }

    #[test]
    fn repeated_calls_do_not_leak_handles() {
        // GDI 핸들 수가 호출마다 늘지 않는지 확인한다(누수 회귀 방지).
        let before = gdi_handle_count();
        for i in 0..200 {
            let _ = rasterize(&format!("{i}"), 64);
        }
        assert_no_handle_growth(before);
    }

    #[test]
    fn bitmap_from_rgba_rejects_bad_sizes() {
        assert!(bitmap_from_rgba(0, 4, &[]).is_none());
        assert!(bitmap_from_rgba(-1, 4, &[]).is_none());
        assert!(
            bitmap_from_rgba(2, 2, &[0; 8]).is_none(),
            "픽셀 수가 맞지 않으면 거부해야 한다"
        );
    }

    /// 미리보기 비트맵은 만든 쪽이 지운다. 만드는 동안 DC가 새지 않아야 한다.
    #[test]
    fn bitmap_from_rgba_does_not_leak_handles() {
        let rgba = vec![255u8; 2 * 2 * 4];
        let before = gdi_handle_count();
        for _ in 0..50 {
            let bitmap = bitmap_from_rgba(2, 2, &rgba).expect("bitmap");
            let _ = unsafe { DeleteObject(HGDIOBJ(bitmap.0)) };
        }
        assert_no_handle_growth(before);
    }

    /// GDI 핸들 수가 늘지 않았는지 본다.
    ///
    /// 핸들 수는 프로세스 전체 값이라 다른 테스트가 병렬로 글리프를 그리면 잠깐 늘어난다.
    /// 몇 번 다시 재서 계속 높을 때만 실패로 본다(테스트가 서로를 깨지 않게).
    fn assert_no_handle_growth(before: u32) {
        for _ in 0..50 {
            if gdi_handle_count() <= before + 5 {
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        panic!(
            "GDI 핸들이 증가했다: before={before} after={}",
            gdi_handle_count()
        );
    }

    fn gdi_handle_count() -> u32 {
        use windows::Win32::System::Threading::{GR_GDIOBJECTS, GetGuiResources};
        // NULL 핸들은 현재 프로세스를 뜻한다.
        unsafe {
            GetGuiResources(
                windows::Win32::Foundation::HANDLE(std::ptr::null_mut()),
                GR_GDIOBJECTS,
            )
        }
    }
}
