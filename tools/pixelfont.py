#!/usr/bin/env python3
"""1비트 트레이 글리프 표를 만든다.

글리프는 이 파일에 ASCII art로 둔다. Rust 표는 여기서만 생성하므로 글리프를
고치려면 이 파일을 고치고 다시 실행한다.

    python tools/pixelfont.py            # src/icon/glyphs.rs 생성
    python tools/pixelfont.py --check    # 커밋된 파일과 같은지 확인

표는 캡션 줄(3x5)과 값 줄(4x10) 두 벌이다.
단위는 캡션 줄로 올라가므로 값 표에는 숫자와 `.`만 있다. 필요한 문자만 둔다.
"""

from __future__ import annotations

import argparse
import sys
from pathlib import Path

# Windows CI의 콘솔 인코딩(cp1252)에서도 한글 메시지가 죽지 않게 한다.
try:
    sys.stdout.reconfigure(encoding="utf-8", errors="replace")
except (AttributeError, ValueError):
    pass

ROOT = Path(__file__).resolve().parent.parent
TARGET = ROOT / "src" / "icon" / "glyphs.rs"

MAX_WIDTH = 8

# 캡션 줄(3x5). `CPU`/`MEM`과 네트워크의 방향 화살표 + 단위(`↓MB`)를 그린다.
# M은 5px로 넓혀 대각선을 살린다(3px이면 MEM이 HEH로 보인다).
LABEL: dict[str, list[str]] = {
    "C": ["111", "100", "100", "100", "111"],
    "P": ["111", "101", "111", "100", "100"],
    "U": ["101", "101", "101", "101", "111"],
    "M": ["10001", "11011", "10101", "10001", "10001"],
    "E": ["111", "100", "111", "100", "111"],
    "B": ["110", "101", "110", "101", "110"],
    "K": ["101", "101", "110", "101", "101"],
    "G": ["111", "100", "101", "101", "111"],
    "V": ["101", "101", "101", "101", "010"],
    "R": ["110", "101", "110", "101", "101"],
    "\u2193": ["010", "010", "010", "111", "010"],
    "\u2191": ["010", "111", "010", "010", "010"],
    "?": ["111", "001", "011", "000", "010"],
}

# 캡션 아래 값 줄(4x10). 단위는 캡션으로 올라가므로 값은 숫자와 `.`뿐이고
# 3글자 이하라 1px 간격을 둬도 16px에 들어간다(`100`=14px).
VALUE: dict[str, list[str]] = {
    "0": ["0110", "1001", "1001", "1001", "1001", "1001", "1001", "1001", "1001", "0110"],
    "1": ["0010", "0110", "0010", "0010", "0010", "0010", "0010", "0010", "0010", "0111"],
    "2": ["0110", "1001", "0001", "0001", "0010", "0010", "0100", "1000", "1000", "1111"],
    "3": ["1110", "0001", "0001", "0001", "0110", "0001", "0001", "0001", "0001", "1110"],
    "4": ["0001", "0011", "0101", "1001", "1001", "1111", "0001", "0001", "0001", "0001"],
    "5": ["1111", "1000", "1000", "1000", "1110", "0001", "0001", "0001", "0001", "1110"],
    "6": ["0110", "1000", "1000", "1000", "1110", "1001", "1001", "1001", "1001", "0110"],
    "7": ["1111", "0001", "0010", "0010", "0010", "0100", "0100", "0100", "1000", "1000"],
    "8": ["0110", "1001", "1001", "1001", "0110", "1001", "1001", "1001", "1001", "0110"],
    "9": ["0110", "1001", "1001", "1001", "1001", "0111", "0001", "0001", "0001", "0110"],
    ".": ["00", "00", "00", "00", "00", "00", "00", "00", "11", "11"],
    "?": ["0110", "1001", "0001", "0010", "0100", "0100", "0000", "0100", "0000", "0000"],
}

def validate(name: str, table: dict[str, list[str]], height: int) -> None:
    for ch, rows in table.items():
        if len(rows) != height:
            raise SystemExit(f"{name} {ch!r}: 행이 {len(rows)}개, {height}개여야 한다")
        width = len(rows[0])
        if not 1 <= width <= MAX_WIDTH:
            raise SystemExit(f"{name} {ch!r}: 폭 {width}은 1..{MAX_WIDTH} 밖이다")
        for row in rows:
            if len(row) != width:
                raise SystemExit(f"{name} {ch!r}: 행 길이가 {width}가 아니다: {row!r}")
            if set(row) - {"0", "1"}:
                raise SystemExit(f"{name} {ch!r}: 0/1 외 문자가 있다: {row!r}")
    if "?" not in table:
        raise SystemExit(f"{name}: 폴백 '?' 글리프가 없다")


def char_literal(ch: str) -> str:
    if ch == "\\":
        return "'\\\\'"
    if ch == "'":
        return "'\\''"
    return f"'{ch}'"


def emit_table(name: str, table: dict[str, list[str]]) -> list[str]:
    # 표를 한 줄에 한 글리프로 유지한다(rustfmt가 접지 않게).
    lines = ["#[rustfmt::skip]", f"pub static {name}: &[(char, Glyph)] = &["]
    for ch, rows in table.items():
        width = len(rows[0])
        masks = []
        for row in rows:
            value = int(row, 2)
            masks.append(f"0b{value:0{width}b}")
        lines.append(
            f"    ({char_literal(ch)}, Glyph {{ width: {width}, rows: &[{', '.join(masks)}] }}),"
        )
    lines.append("];")
    return lines


def render() -> str:
    validate("LABEL", LABEL, 5)
    validate("VALUE", VALUE, 10)
    lines = [
        "//! 1비트 트레이 글리프 표.",
        "//!",
        "//! `tools/pixelfont.py`가 생성한다. 직접 고치지 않는다.",
        "//! 각 글리프는 폭과 행별 비트마스크이며, 행의 최상위 비트가 왼쪽 픽셀이다.",
        "",
        "#[derive(Debug, Clone, Copy, PartialEq, Eq)]",
        "pub struct Glyph {",
        "    pub width: i32,",
        "    pub rows: &'static [u8],",
        "}",
        "",
        "/// 캡션 줄(3x5). 라벨, 단위, 방향 화살표.",
    ]
    lines += emit_table("LABEL", LABEL)
    lines += ["", "/// 캡션 아래 값 줄(4x10)."]
    lines += emit_table("VALUE", VALUE)
    lines.append("")
    return "\n".join(lines)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true", help="생성 결과가 커밋된 파일과 같은지 확인")
    args = parser.parse_args()

    text = render()
    if args.check:
        try:
            current = TARGET.read_text(encoding="utf-8")
        except FileNotFoundError:
            raise SystemExit(f"{TARGET} 가 없다. python tools/pixelfont.py 를 먼저 실행한다")
        if current != text:
            raise SystemExit(f"{TARGET} 가 생성 결과와 다르다. python tools/pixelfont.py 를 실행한다")
        print(f"{TARGET}: 생성 결과와 같다")
        return 0

    TARGET.parent.mkdir(parents=True, exist_ok=True)
    TARGET.write_text(text, encoding="utf-8")
    print(
        f"{TARGET}: {len(LABEL)} 캡션 + {len(VALUE)} 값 글리프"
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
