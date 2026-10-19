"""PNG을 읽어 영역을 잘라 확대한 BMP를 만든다 (표준 라이브러리만 사용)."""
import struct, sys, zlib

def read_png(path):
    d = open(path, 'rb').read()
    assert d[:8] == b'\x89PNG\r\n\x1a\n'
    pos, idat, w, h, bd, ct = 8, b'', 0, 0, 0, 0
    while pos < len(d):
        ln = struct.unpack('>I', d[pos:pos+4])[0]
        typ = d[pos+4:pos+8]
        data = d[pos+8:pos+8+ln]
        if typ == b'IHDR':
            w, h, bd, ct = struct.unpack('>IIBB', data[:10])
        elif typ == b'IDAT':
            idat += data
        elif typ == b'IEND':
            break
        pos += 12 + ln
    assert bd == 8, f'bit depth {bd}'
    ch = {0: 1, 2: 3, 4: 2, 6: 4}[ct]
    raw = zlib.decompress(idat)
    stride = w * ch
    out = bytearray(h * stride)
    prev = bytearray(stride)
    p = 0
    for y in range(h):
        f = raw[p]; p += 1
        line = bytearray(raw[p:p+stride]); p += stride
        if f == 1:
            for i in range(ch, stride): line[i] = (line[i] + line[i-ch]) & 255
        elif f == 2:
            for i in range(stride): line[i] = (line[i] + prev[i]) & 255
        elif f == 3:
            for i in range(stride):
                a = line[i-ch] if i >= ch else 0
                line[i] = (line[i] + ((a + prev[i]) >> 1)) & 255
        elif f == 4:
            for i in range(stride):
                a = line[i-ch] if i >= ch else 0
                b = prev[i]
                c = prev[i-ch] if i >= ch else 0
                pa, pb, pc = abs(b-c), abs(a-c), abs(a+b-2*c)
                pr = a if (pa <= pb and pa <= pc) else (b if pb <= pc else c)
                line[i] = (line[i] + pr) & 255
        out[y*stride:(y+1)*stride] = line
        prev = line
    return w, h, ch, out

def crop_zoom(path, x0, y0, x1, y1, zoom, out_path):
    w, h, ch, px = read_png(path)
    x1, y1 = min(x1, w), min(y1, h)
    cw, chh = x1 - x0, y1 - y0
    W, H = cw * zoom, chh * zoom
    data = bytearray()
    for y in range(H - 1, -1, -1):
        for x in range(W):
            sx, sy = x0 + x // zoom, y0 + y // zoom
            i = (sy * w + sx) * ch
            r, g, b = px[i], px[i+1], px[i+2]
            data += bytes((b, g, r, 0))
    hdr = b'BM' + struct.pack('<IHHI', 14 + 40 + len(data), 0, 0, 54)
    hdr += struct.pack('<IiiHHIIiiII', 40, W, H, 1, 32, 0, len(data), 0, 0, 0, 0)
    open(out_path, 'wb').write(hdr + data)
    print(f'{out_path}: crop({x0},{y0})-({x1},{y1}) zoom {zoom} -> {W}x{H}')

if __name__ == '__main__':
    a = sys.argv
    crop_zoom(a[1], *map(int, a[2:6]), int(a[6]), a[7])
