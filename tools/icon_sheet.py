"""아이콘 비교 시트를 만든다.

두 가지 사용법이 있다.

    # 표준: out/<레이아웃>_<TAG>@<zoom>x.bmp 를 행으로 묶는다
    python tools/icon_sheet.py out/sheet.bmp 16 dark_px16 dark_px24

    # 특정 파일들을 한 줄로 나열한다 (라벨 후보 비교 등)
    python tools/icon_sheet.py out/labels.bmp 16 --files out/lab_arrow_dn@16x.bmp ...

BMP는 bottom-up이므로 읽을 때 행 순서를 뒤집는다.
"""

import struct
import sys

TAGS = ["CPU", "MEM", "NETD", "NETU", "GPU", "VRM"]
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


def compose(rows_of_files, out_path):
    """각 행에 이미지 목록을 받아 시트로 합친다."""
    cell_w = max(load(p)[0] for row in rows_of_files for p in row)
    cell_h = max(load(p)[1] for row in rows_of_files for p in row)
    cols = max(len(row) for row in rows_of_files)

    width = cols * cell_w + (cols + 1) * SEP
    height = len(rows_of_files) * cell_h + (len(rows_of_files) + 1) * SEP
    buffer = [[(28, 28, 30)] * width for _ in range(height)]

    for r, row in enumerate(rows_of_files):
        oy = SEP + r * (cell_h + SEP)
        for c, path in enumerate(row):
            ox = SEP + c * (cell_w + SEP)
            _, _, pixels = load(path)
            for y, line in enumerate(pixels):
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
    print(f"{out_path}: {width}x{height} ({len(rows_of_files)}행)")


def main(argv):
    out_path, zoom = argv[0], argv[1]
    rest = argv[2:]

    if rest and rest[0] == "--files":
        paths = rest[1:]
        for path in paths:
            try:
                open(path, "rb").close()
            except FileNotFoundError:
                raise SystemExit(f"없는 파일: {path}")
        compose([paths], out_path)
        print("열 순서: " + ", ".join(paths))
        return

    layouts = rest
    if not layouts:
        raise SystemExit("레이아웃 이름이나 --files 목록이 필요하다")
    rows = []
    for layout in layouts:
        row = []
        for tag in TAGS:
            path = f"out/{layout}_{tag}@{zoom}x.bmp"
            try:
                open(path, "rb").close()
            except FileNotFoundError:
                raise SystemExit(f"없는 파일: {path} (먼저 --dump 를 실행)")
            row.append(path)
        rows.append(row)
    compose(rows, out_path)
    print(f"행(위→아래)={', '.join(layouts)} 열={', '.join(TAGS)}")


if __name__ == "__main__":
    main(sys.argv[1:])
