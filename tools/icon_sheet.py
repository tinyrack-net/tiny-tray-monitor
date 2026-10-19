"""레이아웃별 아이콘을 한 장의 비교 시트(BMP)로 합친다.

사용: python tools/icon_sheet.py <출력.bmp> <zoom> <레이아웃 이름...>

`cargo run -- --dump` 가 만든 `out/<레이아웃>_<TAG>@<zoom>x.bmp` 를 읽는다.
행 = 레이아웃, 열 = CPU / MEM / NET.
BMP는 bottom-up이므로 읽을 때 행 순서를 뒤집는다.
"""

import struct
import sys

TAGS = ["CPU", "MEM", "NET"]
SEP = 4


def load(path):
    data = open(path, "rb").read()
    offset = struct.unpack("<I", data[10:14])[0]
    width, height = struct.unpack("<ii", data[18:26])
    rows = [
        [
            tuple(data[offset + ((y * width + x) * 4): offset + ((y * width + x) * 4) + 3])
            for x in range(width)
        ]
        for y in range(height - 1, -1, -1)
    ]
    return width, height, rows


def main(out_path, zoom, layouts):
    cell_w, cell_h = 0, 0
    loaded = {}
    for layout in layouts:
        for tag in TAGS:
            path = f"out/{layout}_{tag}@{zoom}x.bmp"
            try:
                width, height, rows = load(path)
            except FileNotFoundError:
                raise SystemExit(f"없는 파일: {path} (먼저 --dump 를 실행)")
            cell_w, cell_h = max(cell_w, width), max(cell_h, height)
            loaded[(layout, tag)] = rows

    cols = len(TAGS)
    width = cols * cell_w + (cols + 1) * SEP
    height = len(layouts) * cell_h + (len(layouts) + 1) * SEP
    buffer = [[(28, 28, 30)] * width for _ in range(height)]

    for row, layout in enumerate(layouts):
        oy = SEP + row * (cell_h + SEP)
        for col, tag in enumerate(TAGS):
            ox = SEP + col * (cell_w + SEP)
            for y, line in enumerate(loaded[(layout, tag)]):
                for x, (b, g, r) in enumerate(line):
                    buffer[oy + y][ox + x] = (b, g, r)

    out = bytearray()
    for y in range(height - 1, -1, -1):
        for x in range(width):
            b, g, r = buffer[y][x]
            out += bytes((b, g, r, 0))
    header = b"BM" + struct.pack("<IHHI", 14 + 40 + len(out), 0, 0, 54)
    header += struct.pack("<IiiHHIIiiII", 40, width, height, 1, 32, 0, len(out), 0, 0, 0, 0)
    open(out_path, "wb").write(header + out)
    print(f"{out_path}: {width}x{height} 행(위→아래)={', '.join(layouts)} 열={', '.join(TAGS)}")


if __name__ == "__main__":
    main(sys.argv[1], sys.argv[2], sys.argv[3:])
