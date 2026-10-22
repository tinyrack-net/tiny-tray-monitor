//! 트레이 아이콘 렌더링.
//!
//! `Shell_NotifyIcon`은 HICON만 받으므로 숫자를 표시하려면 매 갱신마다
//! 비트맵을 만들어 아이콘을 교체해야 한다.
//!
//! - [glyphs]: 1비트 글리프 표(생성물)
//! - [pixel]: 글리프를 1비트로 블릿한다 (순수, GDI 없음)
//! - [render]: `Bitmap`과 배율(nearest-neighbor)
//! - [layout]: master 레이아웃 정책과 지표 정의
//! - [cache]: 표시 문자열 캐시
//!
//! 글리프는 16px master 하나뿐이다. 아이콘 px는 [render::scale_fit]으로 파생한다.

pub mod cache;
pub mod glyphs;
pub mod layout;
pub mod pixel;
pub mod render;

pub use cache::{GlyphCache, GlyphRequest};
pub use layout::{Metric, STACK};
pub use pixel::Face;
pub use render::{Bitmap, Color};

/// 캡션과 값을 위아래로 쌓은 트레이 아이콘.
///
/// 16px master로 그린 뒤 요청한 px에 맞춘다([render::fit]: 정수배 + 여백, 또는 NN). `caption`은 [Metric::caption]이
/// 만든 문자열(라벨·단위)이고 `value`는 숫자만이다. 둘 중 빈 줄은 건너뛴다.
pub fn stacked(
    cache: &mut GlyphCache,
    caption: &str,
    value: &str,
    px: i32,
    label_color: Color,
    value_color: Color,
) -> Bitmap {
    let spec = STACK;
    let mut out = Bitmap::transparent(pixel::MASTER_PX, pixel::MASTER_PX);
    let top = ((pixel::MASTER_PX - spec.block_height()) / 2).max(0);

    if !caption.is_empty() {
        if let Some(bitmap) = cache.get(GlyphRequest {
            text: caption.to_string(),
            face: Face::Label,
            color: label_color,
            alpha: spec.label_alpha,
        }) {
            let x = (pixel::MASTER_PX - bitmap.width) / 2;
            out.blit_over(&bitmap, x, top);
        }
    }

    if !value.is_empty() {
        if let Some(bitmap) = cache.get(GlyphRequest {
            text: value.to_string(),
            face: Face::Value,
            color: value_color,
            alpha: 255,
        }) {
            let x = (pixel::MASTER_PX - bitmap.width) / 2;
            out.blit_over(&bitmap, x, top + spec.label_h + spec.gap);
        }
    }

    render::scale_fit(&out, px.max(1))
}

#[cfg(test)]
mod tests {
    use super::*;

    const WHITE: Color = Color(255, 255, 255);

    fn white(cache: &mut GlyphCache, caption: &str, value: &str, px: i32) -> Bitmap {
        stacked(cache, caption, value, px, WHITE, WHITE)
    }

    /// 줄 범위 안의 최대 알파.
    fn peak(bmp: &Bitmap, from: i32, to: i32) -> u8 {
        (from..to)
            .flat_map(|y| (0..bmp.width).map(move |x| (x, y)))
            .map(|(x, y)| bmp.alpha_at(x, y))
            .max()
            .unwrap_or(0)
    }

    #[test]
    fn stacked_icon_has_caption_and_value_ink() {
        let mut cache = GlyphCache::new(32);
        let bmp = white(&mut cache, "CPU", "80", 16);
        assert_eq!((bmp.width, bmp.height), (16, 16));

        let top = (16 - STACK.block_height()) / 2;
        let value_top = top + STACK.label_h + STACK.gap;
        assert!(
            peak(&bmp, top, top + STACK.label_h) > 0,
            "캡션 줄에 잉크가 없다"
        );
        assert!(
            peak(&bmp, value_top, value_top + STACK.value_h) > 0,
            "값 줄에 잉크가 없다"
        );
    }

    #[test]
    fn caption_is_dimmer_than_value() {
        let mut cache = GlyphCache::new(32);
        let bmp = white(&mut cache, "↓MB", "77", 16);
        let top = (16 - STACK.block_height()) / 2;
        assert_eq!(peak(&bmp, top, top + STACK.label_h), STACK.label_alpha);
        assert_eq!(peak(&bmp, top + STACK.label_h + STACK.gap, 16), 255);
    }

    #[test]
    fn repeated_renders_hit_the_cache() {
        let mut cache = GlyphCache::new(32);
        for _ in 0..20 {
            let _ = white(&mut cache, "CPU", "80", 16);
        }
        let stats = cache.stats();
        assert_eq!(stats.entries, 2, "캡션과 값 두 개만 캐시된다");
        assert!(
            stats.hit_ratio() > 80.0,
            "재사용률이 높아야 한다: {:.1}%",
            stats.hit_ratio()
        );
    }

    #[test]
    fn empty_value_still_renders_the_caption() {
        let mut cache = GlyphCache::new(32);
        let bmp = white(&mut cache, "↓B", "", 16);
        assert!(bmp.rgba.iter().any(|&v| v > 0));
    }

    /// 배율이 달라도 1비트여야 한다(회색이 섞이면 안 된다).
    #[test]
    fn every_size_stays_binary() {
        for px in [16, 20, 24, 28, 32, 36, 40, 44, 48, 52, 56] {
            let mut cache = GlyphCache::new(32);
            let bmp = white(&mut cache, &Metric::NetworkDown.caption("MB"), "100", px);
            assert_eq!((bmp.width, bmp.height), (px, px));
            for pixel in bmp.rgba.chunks_exact(4) {
                assert!(
                    matches!(pixel[3], 0 | 190 | 255),
                    "px={px}: 알파 {}",
                    pixel[3]
                );
            }
        }
    }

    /// 가로·세로로 잇닿은 잉크 run의 길이 목록.
    fn runs(bmp: &Bitmap) -> Vec<i32> {
        let mut out = Vec::new();
        for horizontal in [true, false] {
            for line in 0..bmp.height {
                let mut run = 0;
                for step in 0..=bmp.width {
                    let (x, y) = if horizontal {
                        (step, line)
                    } else {
                        (line, step)
                    };
                    if step < bmp.width && bmp.alpha_at(x, y) > 0 {
                        run += 1;
                    } else if run > 0 {
                        out.push(run);
                        run = 0;
                    }
                }
            }
        }
        out
    }

    /// 2배 이상 들어가는 크기(225%=36px 등)는 정수배로 그려 획이 고르다.
    /// 모든 run이 배율의 배수여야 한다(2.25배 NN이면 2와 3이 섞인다).
    #[test]
    fn large_sizes_keep_uniform_strokes() {
        for px in [32, 36, 40, 44, 48, 52, 56, 64] {
            let scale = px / pixel::MASTER_PX;
            for metric in Metric::ALL {
                for unit in metric.units() {
                    let mut cache = GlyphCache::new(32);
                    let bmp = white(&mut cache, &metric.caption(unit), "1.8", px);
                    assert_eq!((bmp.width, bmp.height), (px, px));
                    for run in runs(&bmp) {
                        assert_eq!(run % scale, 0, "px={px} {metric:?}: run {run}");
                    }
                }
            }
        }
    }

    /// 여백을 둔 크기에서도 블록은 가운데에 있다.
    #[test]
    fn padded_icons_are_centred() {
        let mut cache = GlyphCache::new(32);
        let bmp = white(&mut cache, "MEM", "100", 36);
        let ink_rows: Vec<i32> = (0..36)
            .filter(|&y| (0..36).any(|x| bmp.alpha_at(x, y) > 0))
            .collect();
        assert_eq!(ink_rows.first(), Some(&2), "위 여백 2px");
        assert_eq!(ink_rows.last(), Some(&33), "아래 여백 2px");
    }

    #[test]
    fn every_metric_renders() {
        let mut cache = GlyphCache::new(32);
        for metric in Metric::ALL {
            let bmp = white(&mut cache, &metric.caption(metric.base_unit()), "24", 16);
            assert!(bmp.rgba.iter().any(|&v| v > 0), "{metric:?}");
        }
    }

    /// 가장 넓은 캡션과 값이 16px 상자 안에 들어간다.
    #[test]
    fn captions_and_values_stay_inside_the_icon() {
        for metric in Metric::ALL {
            for unit in metric.units() {
                let caption = metric.caption(unit);
                assert!(pixel::measure(&caption, Face::Label) <= pixel::MASTER_PX);
            }
        }
        for value in ["100", "999", "1.2"] {
            assert!(pixel::measure(value, Face::Value) <= pixel::MASTER_PX);
        }
    }
}
