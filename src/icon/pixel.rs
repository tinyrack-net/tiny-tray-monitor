//! 1비트 픽셀 폰트 렌더러.
//!
//! 글리프 표([super::glyphs])를 직접 블릿한다. GDI를 쓰지 않으므로 안티에일리어싱이
//! 없고, 같은 입력이면 OS와 무관하게 같은 픽셀이 나온다. 그래서 픽셀 단위 테스트가
//! 가능하다.
//!
//! 글리프는 16px master 하나뿐이다. 배율은 master를 정수배(+여백) 또는 NN으로 늘리는
//! [super::render::scale_fit]이 담당한다.

use super::glyphs::{self, Glyph};
use super::render::{Bitmap, Color};

/// 글리프 사이 간격(px, master 기준).
pub const TRACKING: i32 = 1;

/// 아이콘 master 크기(px). 모든 배율은 이 master에서 파생한다.
pub const MASTER_PX: i32 = 16;

/// 글리프 표 두 벌.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Face {
    /// 캡션 줄(3x5). 라벨, 단위, 방향 화살표.
    Label,
    /// 캡션 아래 값 줄(4x10). 숫자와 `.`만 있다.
    Value,
}

impl Face {
    pub fn table(self) -> &'static [(char, Glyph)] {
        match self {
            Face::Label => glyphs::LABEL,
            Face::Value => glyphs::VALUE,
        }
    }

    /// 줄 높이(px).
    pub const fn height(self) -> i32 {
        match self {
            Face::Label => 5,
            Face::Value => 10,
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
    glyph_width + TRACKING * (glyphs.len() as i32 - 1)
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
        pen += glyph.width + TRACKING;
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::icon::Metric;

    const WHITE: Color = Color(255, 255, 255);

    /// 아이콘이 실제로 쓰는 캡션 전부.
    fn every_caption() -> Vec<String> {
        Metric::ALL
            .iter()
            .flat_map(|metric| metric.units().iter().map(|unit| metric.caption(unit)))
            .collect()
    }

    #[test]
    fn widths_match_the_glyph_advances() {
        assert_eq!(measure("CPU", Face::Label), 3 * 3 + 2);
        assert_eq!(measure("MEM", Face::Label), 5 + 3 + 5 + 2);
        assert_eq!(measure("↓MB", Face::Label), 3 + 5 + 3 + 2);
        assert_eq!(measure("80", Face::Value), 4 + 4 + 1);
        assert_eq!(measure("100", Face::Value), 4 * 3 + 2);
        assert_eq!(measure("1.2", Face::Value), 4 + 2 + 4 + 2);
    }

    /// 캡션과 값의 가장 넓은 실제 문자열이 16px 안에 들어가야 한다.
    #[test]
    fn widest_strings_fit_the_master() {
        for caption in every_caption() {
            let width = measure(&caption, Face::Label);
            assert!(width <= MASTER_PX, "캡션 {caption} = {width}px");
        }
        for value in ["100", "999", "1.2", "512", "24", "0"] {
            let width = measure(value, Face::Value);
            assert!(width <= MASTER_PX, "값 {value} = {width}px");
        }
    }

    #[test]
    fn caption_and_value_fill_the_master() {
        assert!(Face::Value.height() > Face::Label.height());
        assert_eq!(
            Face::Label.height() + TRACKING + Face::Value.height(),
            MASTER_PX,
            "캡션과 값이 16px을 꽉 채운다"
        );
    }

    #[test]
    fn render_line_has_the_expected_shape() {
        let bitmap = render_line("80", Face::Value, WHITE, 255).expect("render");
        assert_eq!(
            (bitmap.width, bitmap.height),
            (measure("80", Face::Value), 10)
        );
        assert!(bitmap.rgba.iter().any(|&v| v > 0), "잉크가 있어야 한다");
    }

    /// 픽셀 폰트는 1비트다. 알파가 0 또는 지정한 값만 나와야 한다(회색 금지).
    #[test]
    fn alpha_is_binary() {
        for (text, face, alpha) in [
            ("CPU", Face::Label, 190),
            ("↓MB", Face::Label, 190),
            ("77", Face::Value, 255),
            ("1.2", Face::Value, 255),
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
        for caption in every_caption() {
            for ch in caption.chars() {
                assert!(
                    Face::Label.table().iter().any(|(c, _)| *c == ch),
                    "캡션 {ch:?} 글리프 누락"
                );
            }
        }
        for ch in "0123456789.".chars() {
            assert!(
                Face::Value.table().iter().any(|(c, _)| *c == ch),
                "값 {ch:?} 글리프 누락"
            );
        }
    }

    #[test]
    fn tables_are_well_formed() {
        for face in [Face::Label, Face::Value] {
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
