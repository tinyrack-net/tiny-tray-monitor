//! 설정 창의 아이콘 미리보기.
//!
//! 트레이 아이콘과 같은 렌더러를 쓰되 **배경 위에 합성해 불투명 비트맵**으로 만든다.
//! 정적 컨트롤(`SS_BITMAP`)은 알파를 합성하지 않고 비트를 그대로 복사하기 때문이다.
//! 값은 실제 값이 아니라 표본이다 — 창은 값을 계산하지 않고 표시 폭만 보여준다.

use crate::icon::{self, Bitmap, Color, GlyphCache, Metric};
use crate::settings::model::SettingsDraft;
use crate::theme::Palette;

/// 확대 배율. 16px 아이콘을 그대로 보여주면 라벨을 읽을 수 없다.
pub const SCALE: i32 = 3;

/// 지표별 표본 값(숫자, 단위). 아이콘에서 값이 차지하는 폭을 보여주는 것이 목적이다.
/// 단위는 캡션 줄에 들어간다.
fn sample(metric: Metric) -> (&'static str, &'static str) {
    match metric {
        Metric::Cpu => ("34", "%"),
        Metric::Memory => ("41", "%"),
        Metric::NetworkDown => ("1.7", "KB"),
        Metric::NetworkUp => ("341", "KB"),
        Metric::Gpu => ("14", "%"),
        Metric::GpuVram => ("75", "%"),
    }
}

/// 미리보기 크기(px). 지표 수에 따라 가로로 늘어난다.
pub fn layout(count: usize, icon: i32, gap: i32, padding: i32) -> (i32, i32) {
    let count = count as i32;
    if count == 0 {
        return (padding * 2 + icon, padding * 2 + icon);
    }
    let width = padding * 2 + count * icon + (count - 1) * gap;
    (width, padding * 2 + icon)
}

/// 글리프 캐시를 따로 둔다. 트레이 캐시를 공유하면 하트비트 로그의 히트율이 의미를 잃는다.
pub struct Preview {
    cache: GlyphCache,
}

impl Default for Preview {
    fn default() -> Self {
        // 지표 4개 × 글리프 2개면 충분하다.
        Self {
            cache: GlyphCache::new(16),
        }
    }
}

impl Preview {
    /// 표시 중인 지표만 그린다.
    pub fn strip(
        &mut self,
        draft: &SettingsDraft,
        palette: Palette,
        px: i32,
        scale: i32,
    ) -> Bitmap {
        let scale = scale.max(1);
        let icon = (px * scale).max(1);
        let gap = scale;
        let padding = 2 * scale;

        let metrics: Vec<Metric> = Metric::ALL
            .into_iter()
            .filter(|metric| draft.show.get(*metric))
            .collect();
        let (width, height) = layout(metrics.len(), icon, gap, padding);

        let mut out = Bitmap::transparent(width, height);
        fill(&mut out, palette.taskbar());

        for (index, metric) in metrics.iter().enumerate() {
            let (label_color, value_color) = palette.colors(draft.color_mode, *metric);
            let (number, unit) = sample(*metric);
            let glyph = icon::stacked(
                &mut self.cache,
                &metric.caption(unit),
                number,
                px,
                label_color,
                value_color,
            );
            let x = padding + index as i32 * (icon + gap);
            composite(&mut out, &glyph, scale, x, padding);
        }

        out
    }
}

/// 불투명 배경으로 채운다.
fn fill(bitmap: &mut Bitmap, color: Color) {
    for pixel in bitmap.rgba.chunks_exact_mut(4) {
        pixel.copy_from_slice(&[color.0, color.1, color.2, 255]);
    }
}

/// `source`를 `scale`배로 키워(최근접 이웃) 배경 위에 알파 합성한다.
fn composite(bitmap: &mut Bitmap, source: &Bitmap, scale: i32, x: i32, y: i32) {
    if scale < 1 {
        return;
    }

    for source_y in 0..source.height {
        for source_x in 0..source.width {
            let from = ((source_y * source.width + source_x) * 4) as usize;
            let alpha = source.rgba[from + 3] as u32;
            if alpha == 0 {
                continue;
            }

            for offset_y in 0..scale {
                for offset_x in 0..scale {
                    let to_x = x + source_x * scale + offset_x;
                    let to_y = y + source_y * scale + offset_y;
                    if to_x < 0 || to_y < 0 || to_x >= bitmap.width || to_y >= bitmap.height {
                        continue;
                    }

                    let to = ((to_y * bitmap.width + to_x) * 4) as usize;
                    for channel in 0..3 {
                        let background = bitmap.rgba[to + channel] as u32;
                        let foreground = source.rgba[from + channel] as u32;
                        bitmap.rgba[to + channel] =
                            ((foreground * alpha + background * (255 - alpha)) / 255) as u8;
                    }
                    bitmap.rgba[to + 3] = 255;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use crate::icon::Metric;

    fn draft() -> SettingsDraft {
        SettingsDraft::from_config(&Config::default(), false)
    }

    #[test]
    fn layout_grows_with_the_metric_count() {
        let one = layout(1, 48, 3, 6);
        let four = layout(4, 48, 3, 6);
        assert_eq!(one, (60, 60));
        assert_eq!(four, (4 * 48 + 3 * 3 + 12, 60));
        assert!(four.0 > one.0);

        let none = layout(0, 48, 3, 6);
        assert_eq!(none.1, 60, "지표가 없어도 같은 높이를 유지한다");
    }

    /// 정적 컨트롤은 알파를 합성하지 않으므로 결과에 투명 픽셀이 남으면 안 된다.
    #[test]
    fn strip_is_fully_opaque() {
        let mut preview = Preview::default();
        for palette in [Palette::dark(), Palette::light()] {
            let bitmap = preview.strip(&draft(), palette, 16, SCALE);
            assert_eq!(
                bitmap.rgba.len(),
                (bitmap.width * bitmap.height * 4) as usize
            );
            assert!(
                bitmap.rgba.chunks_exact(4).all(|pixel| pixel[3] == 255),
                "미리보기에는 투명 픽셀이 없어야 한다"
            );

            let background = palette.taskbar();
            let corner = &bitmap.rgba[..4];
            assert_eq!(
                [corner[0], corner[1], corner[2]],
                [background.0, background.1, background.2],
                "가장자리는 작업 표시줄 배경색이다"
            );
        }
    }

    /// 꺼진 지표는 자리를 차지하지 않는다.
    #[test]
    fn strip_only_draws_visible_metrics() {
        let mut preview = Preview::default();
        let all = preview.strip(&draft(), Palette::dark(), 16, SCALE);

        let mut only_cpu = draft();
        for metric in Metric::ALL.into_iter().skip(1) {
            only_cpu.show.set(metric, false);
        }
        let single = preview.strip(&only_cpu, Palette::dark(), 16, SCALE);

        assert!(single.width < all.width, "지표가 줄면 폭도 줄어든다");
        assert_eq!(single.height, all.height);
    }

    /// 확대는 최근접 이웃이므로 3배로 키운 픽셀은 3x3 블록이 된다.
    #[test]
    fn strip_upscales_by_the_scale() {
        let mut preview = Preview::default();
        let small = preview.strip(&draft(), Palette::dark(), 16, 1);
        let large = preview.strip(&draft(), Palette::dark(), 16, 3);

        let expected = layout(Metric::ALL.len(), 16, 1, 2);
        assert_eq!((small.width, small.height), expected);

        let expected = layout(Metric::ALL.len(), 48, 3, 6);
        assert_eq!((large.width, large.height), expected);
    }

    /// 반투명 글리프 픽셀은 배경과 섞여야 한다.
    #[test]
    fn composite_blends_with_the_background() {
        let mut destination = Bitmap::transparent(2, 2);
        fill(&mut destination, Color(0, 0, 0));

        let mut source = Bitmap::transparent(1, 1);
        source.rgba.copy_from_slice(&[255, 255, 255, 128]);

        composite(&mut destination, &source, 2, 0, 0);

        let blended = destination.rgba[0];
        assert!(
            (120..=136).contains(&blended),
            "알파 128이면 절반쯤 밝아야 한다: {blended}"
        );
        assert_eq!(destination.rgba[3], 255);
    }
}
