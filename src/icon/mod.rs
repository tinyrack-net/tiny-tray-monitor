//! 트레이 아이콘 렌더링.
//!
//! `Shell_NotifyIcon`은 HICON만 받으므로 숫자를 표시하려면 매 갱신마다
//! 비트맵을 만들어 아이콘을 교체해야 한다.
//!
//! - [gdi]: GDI로 텍스트를 커버리지 버퍼에 그린다 (unsafe 격리)
//! - [render]: 잘라내기와 다운샘플 (순수)
//! - [layout]: px별 레이아웃 정책 (순수)
//! - [cache]: 표시 문자열 캐시

pub mod cache;
pub mod gdi;
pub mod layout;
pub mod render;

pub use cache::{GlyphCache, GlyphRequest};
pub use layout::{Metric, StackSpec, stack_spec};
pub use render::{Bitmap, Color, Fit};

/// 라벨과 값을 위아래로 쌓은 트레이 아이콘.
///
/// 값이 비어 있으면 값 줄은 건너뛴다. 라벨을 문자열로 받는 이유는 표기 후보를
/// 비교해 보기 위해서다(`--dump`).
pub fn stacked(
    cache: &mut GlyphCache,
    label: &str,
    value: &str,
    px: i32,
    spec: StackSpec,
    label_color: Color,
    value_color: Color,
) -> Bitmap {
    let px = px.max(1);
    let mut out = Bitmap::transparent(px, px);
    let top = ((px - spec.block_height()) / 2).max(0);

    if spec.label_h > 0 {
        if let Some(bitmap) = cache.get(GlyphRequest {
            text: label.to_string(),
            box_w: px,
            box_h: spec.label_h,
            fit: Fit::Uniform,
            color: label_color,
            alpha: spec.label_alpha,
        }) {
            out.blit_over(&bitmap, 0, top);
        }
    }

    if !value.is_empty() && spec.value_h > 0 {
        if let Some(bitmap) = cache.get(GlyphRequest {
            text: value.to_string(),
            box_w: px,
            box_h: spec.value_h,
            fit: spec.value_fit,
            color: value_color,
            alpha: 255,
        }) {
            out.blit_over(&bitmap, 0, top + spec.label_h + spec.gap);
        }
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stacked_icon_has_label_and_value_ink() {
        let mut cache = GlyphCache::new(32);
        let spec = stack_spec(16);
        let bmp = stacked(
            &mut cache,
            Metric::Cpu.label(),
            "80%",
            16,
            spec,
            Color::WHITE,
            Color::WHITE,
        );
        assert_eq!((bmp.width, bmp.height), (16, 16));
        assert!(bmp.rgba.iter().any(|&v| v > 0), "잉크가 있어야 한다");

        // 라벨 줄과 값 줄 모두 잉크가 있어야 한다
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
        let spec = stack_spec(16);
        let bmp = stacked(
            &mut cache,
            Metric::Memory.label(),
            "77%",
            16,
            spec,
            Color::WHITE,
            Color::WHITE,
        );
        let top = (16 - spec.block_height()) / 2;
        let max = |from: i32, to: i32| {
            (from..to)
                .flat_map(|y| (0..16).map(move |x| (x, y)))
                .map(|(x, y)| bmp.alpha_at(x, y))
                .max()
                .unwrap_or(0)
        };
        let label_peak = max(top, top + spec.label_h);
        let value_peak = max(top + spec.label_h + spec.gap, 16);
        assert!(
            label_peak < value_peak,
            "라벨이 값보다 뒤로 물러나야 한다: {label_peak} vs {value_peak}"
        );
    }

    #[test]
    fn repeated_renders_hit_the_cache() {
        let mut cache = GlyphCache::new(32);
        let spec = stack_spec(16);
        for _ in 0..20 {
            let _ = stacked(
                &mut cache,
                Metric::Cpu.label(),
                "80%",
                16,
                spec,
                Color::WHITE,
                Color::WHITE,
            );
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
        let spec = stack_spec(16);
        let bmp = stacked(
            &mut cache,
            Metric::NetworkDown.label(),
            "",
            16,
            spec,
            Color::WHITE,
            Color::WHITE,
        );
        assert!(bmp.rgba.iter().any(|&v| v > 0));
    }
}
