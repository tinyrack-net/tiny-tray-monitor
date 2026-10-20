//! 1비트 트레이 글리프 표.
//!
//! `tools/pixelfont.py`가 생성한다. 직접 고치지 않는다.
//! 각 글리프는 폭과 행별 비트마스크이며, 행의 최상위 비트가 왼쪽 픽셀이다.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Glyph {
    pub width: i32,
    pub rows: &'static [u8],
}

/// 캡션 줄(3x5). 라벨, 단위, 방향 화살표.
#[rustfmt::skip]
pub static LABEL: &[(char, Glyph)] = &[
    ('C', Glyph { width: 3, rows: &[0b111, 0b100, 0b100, 0b100, 0b111] }),
    ('P', Glyph { width: 3, rows: &[0b111, 0b101, 0b111, 0b100, 0b100] }),
    ('U', Glyph { width: 3, rows: &[0b101, 0b101, 0b101, 0b101, 0b111] }),
    ('M', Glyph { width: 5, rows: &[0b10001, 0b11011, 0b10101, 0b10001, 0b10001] }),
    ('E', Glyph { width: 3, rows: &[0b111, 0b100, 0b111, 0b100, 0b111] }),
    ('B', Glyph { width: 3, rows: &[0b110, 0b101, 0b110, 0b101, 0b110] }),
    ('K', Glyph { width: 3, rows: &[0b101, 0b101, 0b110, 0b101, 0b101] }),
    ('G', Glyph { width: 3, rows: &[0b111, 0b100, 0b101, 0b101, 0b111] }),
    ('V', Glyph { width: 3, rows: &[0b101, 0b101, 0b101, 0b101, 0b010] }),
    ('R', Glyph { width: 3, rows: &[0b110, 0b101, 0b110, 0b101, 0b101] }),
    ('↓', Glyph { width: 3, rows: &[0b010, 0b010, 0b010, 0b111, 0b010] }),
    ('↑', Glyph { width: 3, rows: &[0b010, 0b111, 0b010, 0b010, 0b010] }),
    ('?', Glyph { width: 3, rows: &[0b111, 0b001, 0b011, 0b000, 0b010] }),
];

/// 캡션 아래 값 줄(4x10).
#[rustfmt::skip]
pub static VALUE: &[(char, Glyph)] = &[
    ('0', Glyph { width: 4, rows: &[0b0110, 0b1001, 0b1001, 0b1001, 0b1001, 0b1001, 0b1001, 0b1001, 0b1001, 0b0110] }),
    ('1', Glyph { width: 4, rows: &[0b0010, 0b0110, 0b0010, 0b0010, 0b0010, 0b0010, 0b0010, 0b0010, 0b0010, 0b0111] }),
    ('2', Glyph { width: 4, rows: &[0b0110, 0b1001, 0b0001, 0b0001, 0b0010, 0b0010, 0b0100, 0b1000, 0b1000, 0b1111] }),
    ('3', Glyph { width: 4, rows: &[0b1110, 0b0001, 0b0001, 0b0001, 0b0110, 0b0001, 0b0001, 0b0001, 0b0001, 0b1110] }),
    ('4', Glyph { width: 4, rows: &[0b0001, 0b0011, 0b0101, 0b1001, 0b1001, 0b1111, 0b0001, 0b0001, 0b0001, 0b0001] }),
    ('5', Glyph { width: 4, rows: &[0b1111, 0b1000, 0b1000, 0b1000, 0b1110, 0b0001, 0b0001, 0b0001, 0b0001, 0b1110] }),
    ('6', Glyph { width: 4, rows: &[0b0110, 0b1000, 0b1000, 0b1000, 0b1110, 0b1001, 0b1001, 0b1001, 0b1001, 0b0110] }),
    ('7', Glyph { width: 4, rows: &[0b1111, 0b0001, 0b0010, 0b0010, 0b0010, 0b0100, 0b0100, 0b0100, 0b1000, 0b1000] }),
    ('8', Glyph { width: 4, rows: &[0b0110, 0b1001, 0b1001, 0b1001, 0b0110, 0b1001, 0b1001, 0b1001, 0b1001, 0b0110] }),
    ('9', Glyph { width: 4, rows: &[0b0110, 0b1001, 0b1001, 0b1001, 0b1001, 0b0111, 0b0001, 0b0001, 0b0001, 0b0110] }),
    ('.', Glyph { width: 2, rows: &[0b00, 0b00, 0b00, 0b00, 0b00, 0b00, 0b00, 0b00, 0b11, 0b11] }),
    ('?', Glyph { width: 4, rows: &[0b0110, 0b1001, 0b0001, 0b0010, 0b0100, 0b0100, 0b0000, 0b0100, 0b0000, 0b0000] }),
];
