//! GDI 헬퍼. 남은 `unsafe`는 이 모듈에만 둔다.
//!
//! 트레이 아이콘 텍스트는 [super::pixel]이 1비트로 그린다. 여기 남은 것은
//! 설정 창 미리보기용 `HBITMAP` 변환 하나뿐이다.

use core::ffi::c_void;
use core::mem::size_of;

use windows::Win32::Graphics::Gdi::{
    BI_RGB, BITMAPINFO, BITMAPINFOHEADER, CreateDIBSection, DIB_RGB_COLORS, DeleteObject, GetDC,
    HBITMAP, HGDIOBJ, ReleaseDC,
};

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
    /// 핸들 수는 프로세스 전체 값이라 다른 테스트가 병렬로 그리면 잠깐 늘어난다.
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
