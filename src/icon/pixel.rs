//! 1비트 픽셀 폰트 렌더러.
//!
//! 글리프 표([super::glyphs])를 직접 블릿한다. GDI를 쓰지 않으므로 안티에일리어싱이
//! 없고, 같은 입력이면 OS와 무관하게 같은 픽셀이 나온다. 그래서 픽셀 단위 테스트가
//! 가능하다.
//!
//! 글리프는 16px master 하나뿐이다. 배율은 master를 정수배 또는 NN으로 늘리는
//! [super::render::scale_nearest]가 담당한다.

use super::glyphs::{self, Glyph};
use super::render::{Bitmap, Color};

/// 글리프 사이 간격(px, master 기준).
pub const TRACKING: i32 = 1;

/// 아이콘 master 크기(px). 모든 배율은 이 master에서 파생한다.
pub const MASTER_PX: i32 = 16;

/// 글리프 표 세 벌.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Face {
    /// 라벨 줄(3x5).
    Label,
    /// 라벨과 함께 쓰는 값 줄(3x9).
    Value,
    /// 라벨을 숨겼을 때 쓰는 값 줄(4x10).
    ValueLarge,
    /// 라벨과 단위를 모두 숨겼을 때 쓰는 확대 값 줄(4x13).
    ValueHuge,
}

impl Face {
    pub fn table(self) -> &'static [(char, Glyph)] {
        match self {
            Face::Label => glyphs::LABEL,
            Face::Value => glyphs::VALUE,
            Face::ValueLarge => glyphs::VALUE_LARGE,
            Face::ValueHuge => glyphs::VALUE_HUGE,
        }
    }

    /// 줄 높이(px).
    pub fn height(self) -> i32 {
        match self {
            Face::Label => 5,
            Face::Value => 9,
            Face::ValueLarge => 10,
            Face::ValueHuge => 13,
        }
    }

    /// 글리프 사이 간격(px). 4글자를 16px에 맞추는 큰 값 글리프만 붙여 쓴다(0).
    pub fn tracking(self) -> i32 {
        match self {
            Face::Label | Face::Value | Face::ValueHuge => TRACKING,
            Face::ValueLarge => 0,
        }
    }

    /// 글리프를 찾는다. 없으면 `?`로 물러선다.
    pub fn glyph(self, ch: char) -> Option<&'static Glyph> {
        let table = self.table();
        table
            .iter()
            .find(|(c, _)| *c == ch)
            .or_else(|| table.iter().find(|(c, _)| *c == '?'))
            .map(|(_, glyph)| glyph)
    }

    /// 실제로 그려질 글리프 목록(폴백 포함).
    fn glyphs(self, text: &str) -> Vec<&'static Glyph> {
        text.chars().filter_map(|ch| self.glyph(ch)).collect()
    }
}

/// 문자열의 폭(px). 그릴 수 있는 글리프만 센다.
pub fn measure(text: &str, face: Face) -> i32 {
    let glyphs = face.glyphs(text);
    if glyphs.is_empty() {
        return 0;
    }
    let glyph_width: i32 = glyphs.iter().map(|glyph| glyph.width).sum();
    glyph_width + face.tracking() * (glyphs.len() as i32 - 1)
}

/// 한 줄을 잘라낸 비트맵으로 그린다. 그릴 글리프가 없으면 `None`.
pub fn render_line(text: &str, face: Face, color: Color, alpha: u8) -> Option<Bitmap> {
    let glyphs = face.glyphs(text);
    if glyphs.is_empty() {
        return None;
    }
    let width = measure(text, face);
    if width <= 0 {
        return None;
    }
    let mut out = Bitmap::transparent(width, face.height());
    let mut pen = 0;
    for glyph in glyphs {
        for (y, row) in glyph.rows.iter().enumerate() {
            for x in 0..glyph.width {
                if row & (1u8 << (glyph.width - 1 - x)) != 0 {
                    out.set(pen + x, y as i32, color, alpha);
                }
            }
        }
        pen += glyph.width + face.tracking();
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::icon::Metric;

    const WHITE: Color = Color(255, 255, 255);

    #[test]
    fn widths_match_the_glyph_advances() {
        // 2글자, 3글자, 4글자(폭 1인 `.` 포함)를 확인한다.
        assert_eq!(measure("CPU", Face::Label), 3 * 3 + 2);
        assert_eq!(measure("DN", Face::Label), 3 * 2 + 1);
        assert_eq!(measure("80%", Face::Value), 3 + 3 + 4 + 2);
        assert_eq!(measure("24M", Face::Value), 3 * 3 + 2);
        assert_eq!(measure("1.2M", Face::Value), 3 + 1 + 3 + 3 + 3);
    }

    /// 값 줄에서 가장 넓은 실제 문자열이 16px 안에 들어가야 한다.
    #[test]
    fn widest_values_fit_the_master() {
        for value in ["100%", "999K", "1.2G", "1.0K", "512", "0%"] {
            assert!(
                measure(value, Face::Value) <= MASTER_PX,
                "값 {value} = {}px",
                measure(value, Face::Value)
            );
            assert!(
                measure(value, Face::ValueLarge) <= MASTER_PX,
                "큰 값 {value} = {}px",
                measure(value, Face::ValueLarge)
            );
        }
        // 단위를 빼면 3글자라 확대 글리프에도 1px 간격으로 들어간다.
        for value in ["100", "999", "1.2", "512", "24", "0"] {
            assert!(
                measure(value, Face::ValueHuge) <= MASTER_PX,
                "확대 값 {value} = {}px",
                measure(value, Face::ValueHuge)
            );
        }
    }

    #[test]
    fn value_large_face_is_taller_and_wider() {
        assert!(Face::ValueLarge.height() > Face::Value.height());
        assert_eq!(
            Face::ValueLarge.tracking(),
            0,
            "4글자를 16px에 맞추려면 붙여야 한다"
        );
        assert_eq!(Face::ValueHuge.tracking(), 1, "3글자는 간격을 둔다");
        assert!(Face::ValueHuge.height() > Face::ValueLarge.height());
        let digit = Face::ValueLarge.glyph('0').expect("0");
        assert!(digit.width >= Face::Value.glyph('0').expect("0").width);
    }

    #[test]
    fn render_line_has_the_expected_shape() {
        let bitmap = render_line("80%", Face::Value, WHITE, 255).expect("render");
        assert_eq!(
            (bitmap.width, bitmap.height),
            (measure("80%", Face::Value), 9)
        );
        assert!(bitmap.rgba.iter().any(|&v| v > 0), "잉크가 있어야 한다");
    }

    /// 픽셀 폰트는 1비트다. 알파가 0 또는 지정한 값만 나와야 한다(회색 금지).
    #[test]
    fn alpha_is_binary() {
        for (text, face, alpha) in [
            ("CPU", Face::Label, 190),
            ("MEM", Face::Label, 190),
            ("77%", Face::Value, 255),
            ("1.2G", Face::Value, 255),
        ] {
            let bitmap = render_line(text, face, WHITE, alpha).expect("render");
            for pixel in bitmap.rgba.chunks_exact(4) {
                assert!(
                    pixel[3] == 0 || pixel[3] == alpha,
                    "{text}: 알파 {}",
                    pixel[3]
                );
            }
        }
    }

    #[test]
    fn empty_text_renders_nothing() {
        assert!(render_line("", Face::Value, WHITE, 255).is_none());
        assert_eq!(measure("", Face::Value), 0);
    }

    #[test]
    fn unknown_characters_fall_back_to_the_placeholder() {
        let bitmap = render_line("@", Face::Value, WHITE, 255).expect("fallback");
        assert!(bitmap.rgba.iter().any(|&v| v > 0), "'?'로 물러서야 한다");
    }

    /// 앱이 쓰는 모든 문자가 글리프 표에 있어야 한다(폴백으로 가려지면 안 된다).
    #[test]
    fn every_used_character_has_a_glyph() {
        for metric in Metric::ALL {
            for ch in metric.label().chars() {
                assert!(
                    Face::Label.table().iter().any(|(c, _)| *c == ch),
                    "라벨 {ch:?} 글리프 누락"
                );
            }
        }
        for value in ["100%", "0%", "1.2G", "999K", "512", "24M"] {
            for ch in value.chars() {
                for face in [Face::Value, Face::ValueLarge] {
                    assert!(
                        face.table().iter().any(|(c, _)| *c == ch),
                        "{face:?} 값 {ch:?} 글리프 누락"
                    );
                }
            }
        }
        // 단위를 뺀 값은 확대 글리프에도 모든 문자가 있어야 한다.
        for value in ["100", "999", "1.2", "512", "0"] {
            for ch in value.chars() {
                assert!(
                    Face::ValueHuge.table().iter().any(|(c, _)| *c == ch),
                    "확대 값 {ch:?} 글리프 누락"
                );
            }
        }
    }

    #[test]
    fn tables_are_well_formed() {
        for face in [Face::Label, Face::Value, Face::ValueLarge, Face::ValueHuge] {
            for (ch, glyph) in face.table() {
                assert_eq!(
                    glyph.rows.len() as i32,
                    face.height(),
                    "{ch:?} 행 수가 줄 높이와 다르다"
                );
                for row in glyph.rows {
                    // 폭보다 큰 비트는 서 있어선 안 된다(행 정의 실수 방지).
                    assert_eq!(
                        *row & !((1u16 << glyph.width) - 1) as u8,
                        0,
                        "{ch:?} 폭 {width} 밖 비트",
                        width = glyph.width
                    );
                }
            }
        }
    }
}
