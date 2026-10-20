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
//! - [gdi]: 미리보기용 DIB 변환만 남긴다
//!
//! 글리프는 16px master 하나뿐이다. 아이콘 px는 [render::scale_nearest]로 파생한다.

pub mod cache;
pub mod gdi;
pub mod glyphs;
pub mod layout;
pub mod pixel;
pub mod render;

pub use cache::{GlyphCache, GlyphRequest};
pub use layout::{Metric, StackSpec, stack_spec};
pub use pixel::Face;
pub use render::{Bitmap, Color};

/// 라벨과 값을 위아래로 쌓은 트레이 아이콘.
///
/// 16px master로 그린 뒤 요청한 px로 늘린다. `spec`이 라벨 표시 여부와 값
/// 글리프 표를 정한다. 값이 비어 있으면 값 줄은 건너뛴다.
/// 라벨을 문자열로 받는 이유는 표기 후보를 비교해 보기 위해서다.
pub fn stacked(
    cache: &mut GlyphCache,
    label: &str,
    value: &str,
    px: i32,
    spec: StackSpec,
    label_color: Color,
    value_color: Color,
) -> Bitmap {
    let mut out = Bitmap::transparent(pixel::MASTER_PX, pixel::MASTER_PX);
    let top = ((pixel::MASTER_PX - spec.block_height()) / 2).max(0);

    if spec.show_label {
        if let Some(bitmap) = cache.get(GlyphRequest {
            text: label.to_string(),
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
            face: spec.value_face,
            color: value_color,
            alpha: 255,
        }) {
            let x = (pixel::MASTER_PX - bitmap.width) / 2;
            out.blit_over(&bitmap, x, top + spec.label_h + spec.gap);
        }
    }

    render::scale_nearest(&out, px.max(1))
}

#[cfg(test)]
mod tests {
    use super::*;

    const WHITE: Color = Color(255, 255, 255);

    /// 라벨을 보여줄 때의 아이콘.
    fn labeled(cache: &mut GlyphCache, label: &str, value: &str, px: i32) -> Bitmap {
        stacked(
            cache,
            label,
            value,
            px,
            stack_spec(true, true),
            WHITE,
            WHITE,
        )
    }

    #[test]
    fn stacked_icon_has_label_and_value_ink() {
        let mut cache = GlyphCache::new(32);
        let spec = stack_spec(true, true);
        let bmp = labeled(&mut cache, "CPU", "80%", 16);
        assert_eq!((bmp.width, bmp.height), (16, 16));
        assert!(bmp.rgba.iter().any(|&v| v > 0), "잉크가 있어야 한다");

        let top = (16 - spec.block_height()) / 2;
        let label_ink = (top..top + spec.label_h).any(|y| (0..16).any(|x| bmp.alpha_at(x, y) > 8));
        let value_top = top + spec.label_h + spec.gap;
        let value_ink =
            (value_top..value_top + spec.value_h).any(|y| (0..16).any(|x| bmp.alpha_at(x, y) > 8));
        assert!(label_ink, "라벨 줄에 잉크가 없다");
        assert!(value_ink, "값 줄에 잉크가 없다");
    }

    #[test]
    fn label_is_dimmer_than_value() {
        let mut cache = GlyphCache::new(32);
        let spec = stack_spec(true, true);
        let bmp = labeled(&mut cache, "MEM", "77%", 16);
        let peak = |from: i32, to: i32| {
            (from..to)
                .flat_map(|y| (0..16).map(move |x| (x, y)))
                .map(|(x, y)| bmp.alpha_at(x, y))
                .max()
                .unwrap_or(0)
        };
        let top = (16 - spec.block_height()) / 2;
        let label_peak = peak(top, top + spec.label_h);
        let value_peak = peak(top + spec.label_h + spec.gap, 16);
        assert_eq!(label_peak, spec.label_alpha);
        assert_eq!(value_peak, 255);
        assert!(label_peak < value_peak, "라벨이 값보다 뒤로 물러나야 한다");
    }

    #[test]
    fn repeated_renders_hit_the_cache() {
        let mut cache = GlyphCache::new(32);
        for _ in 0..20 {
            let _ = labeled(&mut cache, "CPU", "80%", 16);
        }
        let stats = cache.stats();
        assert_eq!(stats.entries, 2, "라벨과 값 두 개만 캐시된다");
        assert!(
            stats.hit_ratio() > 80.0,
            "재사용률이 높아야 한다: {:.1}%",
            stats.hit_ratio()
        );
    }

    #[test]
    fn empty_value_still_renders_the_label() {
        let mut cache = GlyphCache::new(32);
        let bmp = labeled(&mut cache, "DN", "", 16);
        assert!(bmp.rgba.iter().any(|&v| v > 0));
    }

    /// 라벨을 숨기면 라벨 색은 하나도 그리지 않고, 값만 그린다.
    #[test]
    fn value_only_mode_drops_the_label() {
        let mut cache = GlyphCache::new(32);
        let red = Color(255, 0, 0);
        let green = Color(0, 255, 0);
        let bmp = stacked(
            &mut cache,
            "CPU",
            "80%",
            16,
            stack_spec(false, true),
            red,
            green,
        );
        let has_red = bmp
            .rgba
            .chunks_exact(4)
            .any(|pixel| pixel[0] > 0 && pixel[1] == 0 && pixel[2] == 0);
        let has_green = bmp.rgba.chunks_exact(4).any(|pixel| pixel[1] > 0);
        assert!(!has_red, "라벨을 그리면 안 된다");
        assert!(has_green, "값은 있어야 한다");
    }

    /// 배율이 달라도 1비트여야 한다(회색이 섞이면 안 된다).
    #[test]
    fn every_size_stays_binary() {
        for show_label in [true, false] {
            for show_units in [true, false] {
                for px in [16, 20, 24, 28, 32, 48] {
                    let mut cache = GlyphCache::new(32);
                    let value = if show_units { "100%" } else { "100" };
                    let bmp = stacked(
                        &mut cache,
                        "CPU",
                        value,
                        px,
                        stack_spec(show_label, show_units),
                        WHITE,
                        WHITE,
                    );
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
        }
    }

    #[test]
    fn every_metric_renders_in_every_mode() {
        for show_label in [true, false] {
            for show_units in [true, false] {
                let mut cache = GlyphCache::new(32);
                let value = if show_units { "24M" } else { "24" };
                for metric in Metric::ALL {
                    let bmp = stacked(
                        &mut cache,
                        metric.label(),
                        value,
                        16,
                        stack_spec(show_label, show_units),
                        WHITE,
                        WHITE,
                    );
                    assert!(bmp.rgba.iter().any(|&v| v > 0), "{metric:?}");
                }
            }
        }
    }
}
