#!/usr/bin/env python3
"""앱 아이콘과 MSIX 자산을 만든다.

외부 패키지 없이 표준 라이브러리만 쓴다. 결과물은 저장소에 커밋하므로
빌드 순서에 의존하지 않는다. 디자인을 바꾸려면 이 파일을 고치고 다시 실행한다.

    python tools/make_assets.py            # 생성
    python tools/make_assets.py --check    # 커밋된 파일과 같은지 확인
"""

from __future__ import annotations

import argparse
import math
import struct
import sys
import zlib
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
ICON = ROOT / "resources" / "app.ico"
MSIX = ROOT / "resources" / "msix"

# 트레이 아이콘과 같은 계열의 색. 배경은 작업 표시줄과 구분되는 어두운 회색이다.
BACKGROUND = (28, 32, 38, 255)
BARS = (
    (61, 132, 198, 255),  # CPU
    (63, 185, 120, 255),  # 메모리
    (232, 148, 58, 255),  # 네트워크
)

SUPERSAMPLE = 4


def _bar_rects(width: int, height: int) -> list[tuple[int, int, int, int, int]]:
    """(x0, y0, x1, y1, color_index) 목록. 슈퍼샘플 좌표계."""
    if width > height * 1.4:
        # Wide 타일: 가운데에 지표 막대를 모은다.
        group = int(width * 0.52)
        left = (width - group) // 2
    else:
        left = width // 5
        group = width - left * 2

    bar_h = max(2, height // 10)
    gap = max(2, height // 12)
    total = bar_h * 3 + gap * 2
    top = (height - total) // 2

    rects = []
    for i in range(3):
        # 55%, 77.5%, 100% 길이. 짧은 막대부터 자란다.
        w = int(group * (0.55 + 0.225 * i))
        y0 = top + i * (bar_h + gap)
        rects.append((left, y0, left + w, y0 + bar_h, i))
    return rects


def _render(width: int, height: int) -> list[list[tuple[int, int, int, int]]]:
    """RGBA 행 목록. 가장자리는 슈퍼샘플 후 평균해 부드럽게 만든다."""
    s = SUPERSAMPLE
    w, h = width * s, height * s
    radius = max(2 * s, min(w, h) // 8)
    bars = _bar_rects(w, h)

    # 행마다 둥근 모서리의 좌우 여백을 미리 계산한다.
    insets = []
    for y in range(h):
        if y < radius:
            dy = radius - y - 0.5
        elif y >= h - radius:
            dy = y - (h - radius) + 0.5
        else:
            dy = 0.0
        inset = 0 if dy == 0.0 else int(round(radius - math.sqrt(max(0.0, radius * radius - dy * dy))))
        insets.append(inset)

    rows = []
    for y in range(h):
        inset = insets[y]
        row = [(0, 0, 0, 0)] * w
        if inset < w - inset:
            row[inset : w - inset] = [BACKGROUND] * (w - inset * 2)
        for x0, y0, x1, y1, idx in bars:
            if y0 <= y < y1:
                row[x0:x1] = [BARS[idx]] * (x1 - x0)
        rows.append(row)

    # 박스 필터. 투명 픽셀의 RGB는 0이므로 알파로 가중 평균한다.
    out = []
    for y in range(height):
        line = []
        for x in range(width):
            r = g = b = a = 0
            for dy in range(s):
                for dx in range(s):
                    pr, pg, pb, pa = rows[y * s + dy][x * s + dx]
                    aw = pa
                    r += pr * aw
                    g += pg * aw
                    b += pb * aw
                    a += pa
            if a == 0:
                line.append((0, 0, 0, 0))
            else:
                line.append((r // a, g // a, b // a, a // (s * s)))
        out.append(line)
    return out


def _png(width: int, height: int) -> bytes:
    raw = bytearray()
    for row in _render(width, height):
        raw.append(0)  # filter: none
        for r, g, b, a in row:
            raw += bytes((r, g, b, a))

    def chunk(kind: bytes, data: bytes) -> bytes:
        return (
            struct.pack(">I", len(data))
            + kind
            + data
            + struct.pack(">I", zlib.crc32(kind + data) & 0xFFFFFFFF)
        )

    return (
        b"\x89PNG\r\n\x1a\n"
        + chunk(b"IHDR", struct.pack(">IIBBBBB", width, height, 8, 6, 0, 0, 0))
        + chunk(b"IDAT", zlib.compress(bytes(raw), 9))
        + chunk(b"IEND", b"")
    )


def _ico(sizes: list[int]) -> bytes:
    images = {size: _png(size, size) for size in sizes}
    header = struct.pack("<HHH", 0, 1, len(sizes))
    offset = len(header) + 16 * len(sizes)
    entries = bytearray()
    data = bytearray()
    for size in sizes:
        png = images[size]
        # 256은 0으로 적는다. ICO 디렉터리 규격이다.
        dim = 0 if size >= 256 else size
        entries += struct.pack("<BBBBHHII", dim, dim, 0, 0, 1, 32, len(png), offset)
        data += png
        offset += len(png)
    return header + bytes(entries) + bytes(data)


def outputs() -> dict[Path, bytes]:
    files: dict[Path, bytes] = {ICON: _ico([16, 20, 24, 32, 40, 48, 64, 128, 256])}
    tiles = {
        "StoreLogo.png": (50, 50),
        "Square44x44Logo.png": (44, 44),
        "Square150x150Logo.png": (150, 150),
        "Wide310x150Logo.png": (310, 150),
        # Store는 축소 표시용 targetsize 자산을 요구한다.
        "Square44x44Logo.targetsize-16.png": (16, 16),
        "Square44x44Logo.targetsize-24.png": (24, 24),
        "Square44x44Logo.targetsize-32.png": (32, 32),
        "Square44x44Logo.targetsize-48.png": (48, 48),
        "Square44x44Logo.targetsize-256.png": (256, 256),
        # 플레이트 없는(배경 투명) 변형.
        "Square44x44Logo.altform-unplated.png": (44, 44),
        "Square44x44Logo.altform-lightunplated.png": (44, 44),
    }
    for name, (width, height) in tiles.items():
        files[MSIX / name] = _png(width, height)
    return files


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true", help="생성 결과가 커밋된 파일과 같은지 확인")
    args = parser.parse_args()

    files = outputs()
    if args.check:
        stale = []
        for path, data in files.items():
            if not path.exists() or path.read_bytes() != data:
                stale.append(path.relative_to(ROOT))
        if stale:
            for path in stale:
                print(f"오래된 자산: {path}", file=sys.stderr)
            print("python tools/make_assets.py로 다시 생성하세요.", file=sys.stderr)
            return 1
        print(f"자산 {len(files)}개가 최신이다.")
        return 0

    for path, data in files.items():
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(data)
        print(f"{path.relative_to(ROOT)} ({len(data)} bytes)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
