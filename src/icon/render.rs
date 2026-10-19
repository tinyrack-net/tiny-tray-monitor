//! 커버리지 -> 아이콘 RGBA 변환.
//!
//! 잘라내기와 다운샘플은 순수 함수라 GDI 없이 테스트할 수 있다.
//! 정사각형 아이콘에서는 **폭에 맞추면 높이가 죽기 때문에** [Fit]으로 정책을 나눈다.

use super::gdi::{self, Coverage};

/// 아이콘 배율. 작은 크기에서 GDI 텍스트 품질이 나쁘므로 크게 그린 뒤 줄인다.
pub const SUPERSAMPLE: i32 = 8;

/// 박스에 맞추는 방식.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Fit {
    /// 비율 유지. 폭이 길면 글리프가 납작해진다.
    Uniform,
    /// 높이를 채우고 가로로 압축한다(늘리지는 않는다).
    Squeeze,
}

/// 아이콘 전경색.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Color(pub u8, pub u8, pub u8);

impl Color {
    /// 테스트에서 쓰는 기준색.
    #[cfg(test)]
    pub const WHITE: Self = Self(255, 255, 255);
}

/// 잉크 영역.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Bounds {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

/// 트레이 아이콘으로 넘길 RGBA 비트맵.
#[derive(Clone, PartialEq, Eq)]
pub struct Bitmap {
    pub width: i32,
    pub height: i32,
    /// 다운샘플 후 실제 글리프 높이(px). 작을수록 읽기 어렵다.
    pub glyph_h: i32,
    pub rgba: Vec<u8>,
}

impl core::fmt::Debug for Bitmap {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Bitmap")
            .field("width", &self.width)
            .field("height", &self.height)
            .field("glyph_h", &self.glyph_h)
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
            glyph_h: 0,
            rgba: vec![0; (w * h * 4) as usize],
        }
    }

    /// 알파 값. 범위를 벗어나면 0. 픽셀 단위 검증용.
    #[cfg(test)]
    pub fn alpha_at(&self, x: i32, y: i32) -> u8 {
        if x < 0 || y < 0 || x >= self.width || y >= self.height {
            return 0;
        }
        self.rgba[((y * self.width + x) * 4 + 3) as usize]
    }

    /// 잉크가 있는 마지막 행 + 1. 실측 글리프 높이 검증용.
    #[cfg(test)]
    pub fn ink_height(&self) -> i32 {
        let mut top = self.height;
        let mut bottom = -1;
        for y in 0..self.height {
            if (0..self.width).any(|x| self.alpha_at(x, y) > 8) {
                top = top.min(y);
                bottom = bottom.max(y);
            }
        }
        if bottom < 0 { 0 } else { bottom - top + 1 }
    }

    /// `src`를 알파 기준으로 덮어 그린다. 글리프끼리는 겹치지 않으므로 max 를 쓴다.
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

/// 잉크(임계값 초과) 영역을 찾는다.
pub fn ink_bounds(cov: &Coverage, threshold: u8) -> Option<Bounds> {
    let mut min_x = cov.width;
    let mut min_y = cov.height;
    let mut max_x = -1;
    let mut max_y = -1;
    for y in 0..cov.height {
        for x in 0..cov.width {
            if cov.get(x, y) > threshold {
                min_x = min_x.min(x);
                min_y = min_y.min(y);
                max_x = max_x.max(x);
                max_y = max_y.max(y);
            }
        }
    }
    (max_x >= 0).then(|| Bounds {
        x: min_x,
        y: min_y,
        width: max_x - min_x + 1,
        height: max_y - min_y + 1,
    })
}

/// 잉크 영역을 목표 박스에 맞춰 축소하고 중앙 정렬한다.
///
/// 원본 영역의 평균 커버리지를 취한 뒤(박스 필터) 최대값으로 정규화해
/// 얇은 획이 회색으로 죽지 않게 한다.
pub fn downsample(
    cov: &Coverage,
    bounds: Bounds,
    box_w: i32,
    box_h: i32,
    fit: Fit,
    color: Color,
    alpha: u8,
) -> Bitmap {
    let box_w = box_w.max(1);
    let box_h = box_h.max(1);
    let ink_w = bounds.width.max(1) as f32;
    let ink_h = bounds.height.max(1) as f32;

    let (dst_w, dst_h) = match fit {
        Fit::Uniform => {
            let scale = (box_w as f32 / ink_w).min(box_h as f32 / ink_h);
            (
                ((ink_w * scale).round() as i32).clamp(1, box_w),
                ((ink_h * scale).round() as i32).clamp(1, box_h),
            )
        }
        Fit::Squeeze => {
            // 높이는 박스를 채우고, 가로는 눌러서 맞춘다. 원본이 좁으면 늘리지 않는다.
            let natural_w = ink_w * box_h as f32 / ink_h;
            ((natural_w.round() as i32).clamp(1, box_w), box_h)
        }
    };

    let mut out = Bitmap::transparent(box_w, box_h);
    let offset_x = (box_w - dst_w) / 2;
    let offset_y = (box_h - dst_h) / 2;

    let mut peak = 0u8;
    let mut samples = vec![0u8; (dst_w * dst_h) as usize];

    for dy in 0..dst_h {
        let sy0 = bounds.y as f32 + dy as f32 * ink_h / dst_h as f32;
        let sy1 = bounds.y as f32 + (dy + 1) as f32 * ink_h / dst_h as f32;
        let iy0 = sy0.floor() as i32;
        let iy1 = (sy1.ceil() as i32).max(iy0 + 1);
        for dx in 0..dst_w {
            let sx0 = bounds.x as f32 + dx as f32 * ink_w / dst_w as f32;
            let sx1 = bounds.x as f32 + (dx + 1) as f32 * ink_w / dst_w as f32;
            let ix0 = sx0.floor() as i32;
            let ix1 = (sx1.ceil() as i32).max(ix0 + 1);

            let mut sum = 0u32;
            let mut count = 0u32;
            for sy in iy0..iy1 {
                for sx in ix0..ix1 {
                    sum += cov.get(sx, sy) as u32;
                    count += 1;
                }
            }
            let value = sum.checked_div(count).unwrap_or(0) as u8;
            peak = peak.max(value);
            samples[(dy * dst_w + dx) as usize] = value;
        }
    }

    // 최대값 기준으로 정규화하고 라벨 밝기를 곱한다.
    let gain = if peak == 0 { 0.0 } else { 255.0 / peak as f32 };
    for dy in 0..dst_h {
        for dx in 0..dst_w {
            let value = samples[(dy * dst_w + dx) as usize] as f32 * gain;
            let value = (value.min(255.0) * alpha as f32 / 255.0).round() as u8;
            if value == 0 {
                continue;
            }
            let index = (((offset_y + dy) * box_w + offset_x + dx) * 4) as usize;
            out.rgba[index] = color.0;
            out.rgba[index + 1] = color.1;
            out.rgba[index + 2] = color.2;
            out.rgba[index + 3] = value;
        }
    }
    out.glyph_h = dst_h;
    out
}

/// 한 줄 텍스트를 GDI로 그려 목표 박스에 맞춘다.
pub fn render_text(
    text: &str,
    box_w: i32,
    box_h: i32,
    fit: Fit,
    color: Color,
    alpha: u8,
) -> Option<Bitmap> {
    let font_px = box_w.max(box_h) * SUPERSAMPLE;
    let cov = gdi::rasterize(text, font_px)?;
    let bounds = ink_bounds(&cov, 8)?;
    Some(downsample(&cov, bounds, box_w, box_h, fit, color, alpha))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 왼쪽 절반만 칠해진 합성 커버리지.
    fn synthetic(width: i32, height: i32, rect: (i32, i32, i32, i32)) -> Coverage {
        let mut data = vec![0u8; (width * height) as usize];
        for y in rect.1..rect.3 {
            for x in rect.0..rect.2 {
                if x >= 0 && y >= 0 && x < width && y < height {
                    data[(y * width + x) as usize] = 255;
                }
            }
        }
        Coverage::new(width, height, data)
    }

    #[test]
    fn ink_bounds_finds_the_drawn_rect() {
        let cov = synthetic(32, 32, (4, 8, 20, 24));
        assert_eq!(
            ink_bounds(&cov, 8),
            Some(Bounds {
                x: 4,
                y: 8,
                width: 16,
                height: 16
            })
        );
    }

    #[test]
    fn ink_bounds_is_none_when_blank() {
        let cov = synthetic(8, 8, (0, 0, 0, 0));
        assert_eq!(ink_bounds(&cov, 8), None);
    }

    #[test]
    fn uniform_fit_preserves_aspect_and_stays_inside_the_box() {
        // 16x64 잉크를 16x16 박스에 넣으면 폭이 4로 줄고 높이는 16을 넘지 않는다
        let cov = synthetic(64, 64, (0, 0, 16, 64));
        let bmp = downsample(
            &cov,
            ink_bounds(&cov, 8).unwrap(),
            16,
            16,
            Fit::Uniform,
            Color::WHITE,
            255,
        );
        assert_eq!((bmp.width, bmp.height), (16, 16));
        assert!(bmp.glyph_h <= 16);
        assert_eq!(bmp.ink_height(), bmp.glyph_h, "높이는 박스를 채운다");
        let ink_w = (0..bmp.width)
            .filter(|&x| (0..bmp.height).any(|y| bmp.alpha_at(x, y) > 8))
            .count() as i32;
        // 원본 16x64 의 비율(1:4)이 유지되어야 한다 -> 폭 약 4
        let ratio = ink_w as f32 / bmp.glyph_h as f32;
        assert!(
            (ratio - 0.25).abs() < 0.08,
            "비율이 유지되어야 한다: {ratio}"
        );
    }

    #[test]
    fn squeeze_fills_height_and_compresses_width() {
        // 가로로 긴 잉크를 16x9 박스에 넣으면 높이를 채우고 가로로 압축된다
        let cov = synthetic(128, 64, (0, 0, 128, 32));
        let bmp = downsample(
            &cov,
            ink_bounds(&cov, 8).unwrap(),
            16,
            9,
            Fit::Squeeze,
            Color::WHITE,
            255,
        );
        assert_eq!(bmp.ink_height(), 9, "높이를 채워야 한다");
        let ink_w = (0..bmp.width)
            .filter(|&x| (0..bmp.height).any(|y| bmp.alpha_at(x, y) > 8))
            .count() as i32;
        assert!(ink_w <= 16);
        assert!(ink_w > 9, "압축은 하되 찌그러질 만큼은 아니다");
    }

    #[test]
    fn squeeze_never_stretches_narrow_ink() {
        // 세로로 긴 잉크(숫자 1 같은)를 넓은 박스에 넣어도 늘리지 않는다
        let cov = synthetic(64, 64, (0, 0, 8, 64));
        let bmp = downsample(
            &cov,
            ink_bounds(&cov, 8).unwrap(),
            16,
            9,
            Fit::Squeeze,
            Color::WHITE,
            255,
        );
        assert_eq!(bmp.ink_height(), 9);
        let ink_w = (0..bmp.width)
            .filter(|&x| (0..bmp.height).any(|y| bmp.alpha_at(x, y) > 8))
            .count() as i32;
        assert!(ink_w <= 3, "늘리면 안 된다: {ink_w}");
    }

    #[test]
    fn alpha_is_normalized_and_scaled() {
        let cov = synthetic(64, 64, (0, 0, 64, 64));
        let bounds = ink_bounds(&cov, 8).unwrap();

        let full = downsample(&cov, bounds, 16, 16, Fit::Uniform, Color::WHITE, 255);
        assert_eq!(full.alpha_at(8, 8), 255);

        let dim = downsample(&cov, bounds, 16, 16, Fit::Uniform, Color::WHITE, 190);
        assert_eq!(dim.alpha_at(8, 8), 190);
    }

    #[test]
    fn color_is_applied() {
        let cov = synthetic(64, 64, (0, 0, 64, 64));
        let bmp = downsample(
            &cov,
            ink_bounds(&cov, 8).unwrap(),
            16,
            16,
            Fit::Uniform,
            Color(10, 20, 30),
            255,
        );
        let index = ((8 * 16) + 8) * 4;
        assert_eq!(&bmp.rgba[index..index + 3], &[10, 20, 30]);
    }

    #[test]
    fn blank_coverage_yields_transparent_bitmap() {
        let cov = synthetic(8, 8, (0, 0, 0, 0));
        assert!(ink_bounds(&cov, 8).is_none());
        let bmp = Bitmap::transparent(16, 16);
        assert!(bmp.rgba.iter().all(|&v| v == 0));
        assert_eq!(bmp.glyph_h, 0);
    }

    #[test]
    fn zero_sized_box_does_not_panic() {
        let cov = synthetic(32, 32, (0, 0, 32, 32));
        let bounds = ink_bounds(&cov, 8).unwrap();
        let bmp = downsample(&cov, bounds, 0, 0, Fit::Uniform, Color::WHITE, 255);
        assert_eq!((bmp.width, bmp.height), (1, 1));
    }

    #[test]
    fn blit_places_glyphs_and_keeps_the_stronger_alpha() {
        let mut base = Bitmap::transparent(16, 16);
        let mut a = Bitmap::transparent(16, 16);
        let mut b = Bitmap::transparent(16, 16);
        // (0, 0) 픽셀의 알파
        a.rgba[3] = 100;
        b.rgba[3] = 200;
        base.blit_over(&a, 0, 0);
        assert_eq!(base.alpha_at(0, 0), 100);
        base.blit_over(&b, 0, 0);
        assert_eq!(base.alpha_at(0, 0), 200, "더 강한 알파가 남는다");
        base.blit_over(&a, 8, 8);
        assert_eq!(base.alpha_at(8, 8), 100);
    }

    #[test]
    fn real_text_render_is_bounded() {
        let bmp = render_text("80%", 16, 9, Fit::Squeeze, Color::WHITE, 255).expect("render");
        assert_eq!((bmp.width, bmp.height), (16, 9));
        assert!(bmp.glyph_h <= 9);
        assert!(bmp.rgba.iter().any(|&v| v > 0), "잉크가 있어야 한다");
    }

    #[test]
    fn empty_text_render_returns_none() {
        assert!(render_text("", 16, 9, Fit::Squeeze, Color::WHITE, 255).is_none());
    }
}
