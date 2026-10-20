//! 아이콘 비트맵과 배율 처리.
//!
//! 픽셀 폰트가 만든 16px master를 목표 크기로 늘린다. 비정수 배율은 라운딩 없이
//! nearest-neighbor로 복제하므로 결과는 항상 1비트다(획 두께만 ±1px 달라진다).
//! 배율마다 글리프를 새로 만들지 않기 위한 장치다.

/// 아이콘 전경색.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Color(pub u8, pub u8, pub u8);

impl Color {
    /// 테스트에서 쓰는 기준색.
    #[cfg(test)]
    pub const WHITE: Self = Self(255, 255, 255);
}

/// 트레이 아이콘으로 넘길 RGBA 비트맵.
#[derive(Clone, PartialEq, Eq)]
pub struct Bitmap {
    pub width: i32,
    pub height: i32,
    pub rgba: Vec<u8>,
}

impl core::fmt::Debug for Bitmap {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Bitmap")
            .field("width", &self.width)
            .field("height", &self.height)
            .finish()
    }
}

impl Bitmap {
    /// 완전히 투명한 비트맵.
    pub fn transparent(width: i32, height: i32) -> Self {
        let (w, h) = (width.max(0), height.max(0));
        Self {
            width: w,
            height: h,
            rgba: vec![0; (w * h * 4) as usize],
        }
    }

    /// 한 픽셀을 덮어쓴다. 범위를 벗어나면 무시한다.
    pub fn set(&mut self, x: i32, y: i32, color: Color, alpha: u8) {
        if x < 0 || y < 0 || x >= self.width || y >= self.height {
            return;
        }
        let index = ((y * self.width + x) * 4) as usize;
        self.rgba[index] = color.0;
        self.rgba[index + 1] = color.1;
        self.rgba[index + 2] = color.2;
        self.rgba[index + 3] = alpha;
    }

    /// 알파 값. 범위를 벗어나면 0. 픽셀 단위 검증용.
    #[cfg(test)]
    pub fn alpha_at(&self, x: i32, y: i32) -> u8 {
        if x < 0 || y < 0 || x >= self.width || y >= self.height {
            return 0;
        }
        self.rgba[((y * self.width + x) * 4 + 3) as usize]
    }

    /// `src`를 알파 기준으로 덮어 그린다. 글리프끼리는 겹치지 않으므로 max를 쓴다.
    pub fn blit_over(&mut self, src: &Bitmap, offset_x: i32, offset_y: i32) {
        for y in 0..src.height {
            let dy = offset_y + y;
            if dy < 0 || dy >= self.height {
                continue;
            }
            for x in 0..src.width {
                let dx = offset_x + x;
                if dx < 0 || dx >= self.width {
                    continue;
                }
                let si = ((y * src.width + x) * 4) as usize;
                let a = src.rgba[si + 3];
                if a == 0 {
                    continue;
                }
                let di = ((dy * self.width + dx) * 4) as usize;
                if a > self.rgba[di + 3] {
                    self.rgba[di] = src.rgba[si];
                    self.rgba[di + 1] = src.rgba[si + 1];
                    self.rgba[di + 2] = src.rgba[si + 2];
                    self.rgba[di + 3] = a;
                }
            }
        }
    }
}

/// 정사각형 `size`로 nearest-neighbor 확대/축소한다.
///
/// 정수배는 픽셀을 그대로 복제하고, 비정수배는 1비트를 유지한 채 획 두께만 ±1px
/// 달라진다. 안티에일리어싱을 하지 않는다.
pub fn scale_nearest(src: &Bitmap, size: i32) -> Bitmap {
    let size = size.max(1);
    let mut out = Bitmap::transparent(size, size);
    if src.width <= 0 || src.height <= 0 || src.rgba.is_empty() {
        return out;
    }
    for y in 0..size {
        let sy = (y * src.height / size).min(src.height - 1);
        for x in 0..size {
            let sx = (x * src.width / size).min(src.width - 1);
            let from = ((sy * src.width + sx) * 4) as usize;
            let to = ((y * size + x) * 4) as usize;
            out.rgba[to..to + 4].copy_from_slice(&src.rgba[from..from + 4]);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dot(x: i32, y: i32, alpha: u8, size: i32) -> Bitmap {
        let mut bitmap = Bitmap::transparent(size, size);
        bitmap.set(x, y, Color::WHITE, alpha);
        bitmap
    }

    #[test]
    fn transparent_bitmap_is_empty() {
        let bitmap = Bitmap::transparent(16, 16);
        assert_eq!((bitmap.width, bitmap.height), (16, 16));
        assert!(bitmap.rgba.iter().all(|&v| v == 0));
    }

    #[test]
    fn set_writes_color_and_alpha() {
        let mut bitmap = Bitmap::transparent(4, 4);
        bitmap.set(2, 1, Color(10, 20, 30), 200);
        assert_eq!(bitmap.alpha_at(2, 1), 200);
        let (x, y) = (2, 1);
        let index = ((y * 4 + x) * 4) as usize;
        assert_eq!(&bitmap.rgba[index..index + 3], &[10, 20, 30]);
        // 범위 밖은 조용히 무시한다.
        bitmap.set(9, 9, Color::WHITE, 255);
        assert_eq!(bitmap.alpha_at(9, 9), 0);
    }

    #[test]
    fn integer_scale_duplicates_pixels() {
        let src = dot(1, 2, 255, 16);
        let scaled = scale_nearest(&src, 32);
        assert_eq!((scaled.width, scaled.height), (32, 32));
        // (1,2)는 (2..3, 4..5) 블록이 된다.
        for y in 4..6 {
            for x in 2..4 {
                assert_eq!(scaled.alpha_at(x, y), 255, "({x},{y})");
            }
        }
        assert_eq!(scaled.alpha_at(0, 0), 0);
    }

    /// 비정수 배율도 1비트를 유지해야 한다(회색이 생기면 안 된다).
    #[test]
    fn non_integer_scale_stays_binary() {
        let mut src = Bitmap::transparent(16, 16);
        for x in 0..16 {
            for y in 0..16 {
                if (x + y) % 3 == 0 {
                    src.set(x, y, Color::WHITE, 190);
                }
            }
        }
        for size in [20, 24, 28, 40] {
            let scaled = scale_nearest(&src, size);
            for pixel in scaled.rgba.chunks_exact(4) {
                assert!(
                    pixel[3] == 0 || pixel[3] == 190,
                    "size={size}: 알파 {}",
                    pixel[3]
                );
            }
        }
    }

    #[test]
    fn scale_to_the_same_size_copies() {
        let src = dot(5, 5, 255, 16);
        assert_eq!(scale_nearest(&src, 16), src);
    }

    #[test]
    fn blit_places_glyphs_and_keeps_the_stronger_alpha() {
        let mut base = Bitmap::transparent(16, 16);
        let a = dot(0, 0, 100, 16);
        let b = dot(0, 0, 200, 16);
        base.blit_over(&a, 0, 0);
        assert_eq!(base.alpha_at(0, 0), 100);
        base.blit_over(&b, 0, 0);
        assert_eq!(base.alpha_at(0, 0), 200, "더 강한 알파가 남는다");
        base.blit_over(&a, 8, 8);
        assert_eq!(base.alpha_at(8, 8), 100);
    }
}
