"""Store 스크린샷(1920x1080)을 언어별로 합성한다.

입력은 모두 실제 앱과 Windows가 만든 픽셀이다.
- `out/screen.png`: 지표 8개를 고정한 작업 표시줄이 보이는 전체 화면을 225% 배율에서
  `tools/capture_screen.ps1`로 캡처한 것. 아래 `TASKBAR`의 좌표는 이 캡처에서 잰 값이다.
- `out/shot-<lang>.png`: 해당 언어의 설정 창을 PrintWindow로 1:1 캡처한 것
  (`resources/store/README.md`의 절차).
- `out/dark_px16_<TAG>@16x.bmp`: `tiny-tray-monitor --dump`가 그린 16px 트레이 아이콘.

결과는 `resources/store/screenshots/<lang>/01-taskbar.png`, `02-settings.png`, `03-tray.png`.

필요: Pillow (`pip install pillow`). 사용: python tools/store_shots.py
"""

import os
import statistics
import sys

from PIL import Image, ImageDraw, ImageFilter, ImageFont

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
OUT = os.path.join(ROOT, "out")
DEST = os.path.join(ROOT, "resources", "store", "screenshots")
FONTS = r"C:\Windows\Fonts"

W, H = 1920, 1080
BG = (28, 28, 28)
FG = (245, 245, 245)
MUTED = (170, 170, 170)

# 트레이 순서는 작업 표시줄에 보이는 순서(오른쪽이 CPU)를 따른다.
TAGS = ["DIO", "DSK", "VRM", "GPU", "NETU", "NETD", "MEM", "CPU"]

# `out/screen.png`(2560x1440, 225%)에서 잰 작업 표시줄 좌표. 다시 캡처하면 다시 잰다.
# 열 범위는 모두 [시작, 끝) 이다.
TASKBAR = {
    "top": 1332,  # 위쪽 테두리 첫 행. 작업 표시줄은 여기서 화면 아래까지 108px
    "border": 2,  # 테두리 두께
    "bg_rows": (1336, 1346),  # 배경색을 재는 빈 행
    # 앱: 시작, 작업 보기, Edge, 터미널, 탐색기, 설정. 개인 앱과 알림 배지는 뺀다.
    # 1920px에 225%면 가운데 정렬로는 트레이와 겹치므로 왼쪽 정렬(Windows 11 설정)로 놓는다.
    "apps": [330, 431, 629, 926, 1124, 1223],
    "app_cell": 99,
    # 트레이: 펼침 단추부터 DIO까지 한 덩어리, 그다음 네트워크·볼륨. IME 표시와 시계는
    # 언어마다 다르므로 뺀다.
    "tray": (1414, 2040),
    "system": (2228, 2348),
    "gap": 33,  # DIO와 네트워크 사이 간격(원래 IME 표시까지의 간격)
    # 트레이 지표 8개(왼쪽 CPU부터)의 열 범위와 행 범위
    "icons": [(1499, 1521), (1567, 1597), (1643, 1665), (1713, 1741),
              (1787, 1809), (1857, 1883), (1931, 1953), (2003, 2025)],
    "icon_rows": (1368, 1404),
}

COPY = {
    "en": {
        "font": ("segoeui.ttf", "segoeuib.ttf"),
        "brand": "TINYTRAYMONITOR",
        "hero": ["Your PC at a glance,", "right in the taskbar"],
        "hero_sub": "CPU, memory, network, GPU and disk as eight tray icons",
        "headline": ["CPU, memory, network,", "GPU and disk", "in your system tray"],
        "bullets": [
            "Eight readouts, one tray icon each",
            "Pick the GPU, network adapter and drive",
            "Colour per readout or taskbar theme",
            "Light, dark or automatic settings window",
            "No account, no telemetry, no network access",
        ],
        "strip": "Lives in your taskbar",
        "title2": "Read the numbers at a glance",
        "sub2": "Eight independent readouts in your system tray",
        "labels": {
            "CPU": "CPU", "MEM": "Memory", "NETD": "Download", "NETU": "Upload",
            "GPU": "GPU", "VRM": "VRAM", "DSK": "Disk usage", "DIO": "Disk activity",
        },
        "foot2": "Turn off the ones you do not need.",
    },
    "ko": {
        "font": ("malgun.ttf", "malgunbd.ttf"),
        "brand": "TINYTRAYMONITOR",
        "hero": ["작업 표시줄에서", "바로 보는 PC 상태"],
        "hero_sub": "CPU, 메모리, 네트워크, GPU, 디스크를 트레이 아이콘 8개로",
        "headline": ["CPU, 메모리, 네트워크,", "GPU와 디스크를", "시스템 트레이에서"],
        "bullets": [
            "8가지 지표, 지표마다 트레이 아이콘 하나",
            "측정할 GPU, 네트워크 어댑터, 드라이브 선택",
            "지표별 색상 또는 작업 표시줄 테마",
            "밝게, 어둡게, 자동 설정 창",
            "계정, 원격 분석, 네트워크 접속 없음",
        ],
        "strip": "작업 표시줄에 그대로",
        "title2": "한눈에 읽는 숫자",
        "sub2": "시스템 트레이의 독립된 지표 8개",
        "labels": {
            "CPU": "CPU", "MEM": "메모리", "NETD": "다운로드", "NETU": "업로드",
            "GPU": "GPU", "VRM": "VRAM", "DSK": "디스크 사용량", "DIO": "디스크 로드율",
        },
        "foot2": "필요 없는 지표는 끌 수 있습니다.",
    },
    "ja": {
        "font": ("YuGothM.ttc", "YuGothB.ttc"),
        "brand": "TINYTRAYMONITOR",
        "hero": ["PC の状態を", "タスクバーでひと目で"],
        "hero_sub": "CPU、メモリ、ネットワーク、GPU、ディスクを 8 つのトレイ アイコンに",
        "headline": ["CPU、メモリ、", "ネットワーク、GPU、", "ディスクをトレイに"],
        "bullets": [
            "8 つの項目を 1 つずつトレイ アイコンに",
            "計測する GPU、アダプター、ドライブを選択",
            "項目ごとの色、またはタスクバーのテーマ",
            "ライト、ダーク、自動の設定ウィンドウ",
            "アカウント不要、テレメトリなし、通信なし",
        ],
        "strip": "タスクバーに常駐",
        "title2": "数値をひと目で",
        "sub2": "システム トレイに並ぶ 8 つの独立した項目",
        "labels": {
            "CPU": "CPU", "MEM": "メモリ", "NETD": "ダウンロード", "NETU": "アップロード",
            "GPU": "GPU", "VRM": "VRAM", "DSK": "ディスク使用率", "DIO": "ディスク負荷",
        },
        "foot2": "不要な項目はオフにできます。",
    },
}


def font(name, size):
    return ImageFont.truetype(os.path.join(FONTS, name), size)


def icon(tag, scale):
    """16px 마스터를 정수 배로 확대한다. 1비트 글꼴이므로 최근접 이웃만 쓴다."""
    src = Image.open(os.path.join(OUT, f"dark_px16_{tag}@16x.bmp")).convert("RGB")
    master = src.resize((16, 16), Image.NEAREST)
    big = master.resize((16 * scale, 16 * scale), Image.NEAREST)
    # 덤프 배경(작업 표시줄 색)을 스크린샷 배경으로 바꾼다. 모서리가 글자일 수 있으므로
    # 가장 많은 색을 배경으로 본다.
    key = max(master.getcolors(256), key=lambda pair: pair[0])[1]
    px = big.load()
    for y in range(big.height):
        for x in range(big.width):
            if px[x, y] == key:
                px[x, y] = BG
    return big


def center_text(draw, y, text, fnt, fill):
    w = draw.textlength(text, font=fnt)
    draw.text(((W - w) / 2, y), text, font=fnt, fill=fill)


def settings_shot(lang, c):
    img = Image.new("RGB", (W, H), BG)
    d = ImageDraw.Draw(img)
    regular, bold = c["font"]

    dialog = Image.open(os.path.join(OUT, f"shot-{lang}.png")).convert("RGB")
    dx, dy = 150, (H - dialog.height) // 2
    img.paste(dialog, (dx, dy))
    d.rectangle([dx - 1, dy - 1, dx + dialog.width, dy + dialog.height], outline=(70, 70, 70))

    x = dx + dialog.width + 110
    y = 250
    d.text((x, y), c["brand"], font=font(regular, 22), fill=MUTED)
    y += 50
    head = font(bold, 54)
    for line in c["headline"]:
        d.text((x, y), line, font=head, fill=FG)
        y += 70
    y += 30
    body = font(regular, 27)
    for bullet in c["bullets"]:
        d.text((x, y), "•", font=body, fill=MUTED)
        d.text((x + 28, y), bullet, font=body, fill=FG)
        y += 44

    y += 40
    d.text((x, y), c["strip"], font=font(regular, 22), fill=MUTED)
    y += 40
    strip = Image.new("RGB", (len(TAGS) * 64 + 24, 72), (32, 32, 32))
    for i, tag in enumerate(TAGS):
        strip.paste(icon(tag, 3), (12 + i * 64 + 8, 12))
    img.paste(strip, (x, y))
    return img


def tray_shot(lang, c):
    img = Image.new("RGB", (W, H), BG)
    d = ImageDraw.Draw(img)
    regular, bold = c["font"]
    center_text(d, 170, c["title2"], font(bold, 60), FG)
    center_text(d, 260, c["sub2"], font(regular, 30), MUTED)

    order = list(reversed(TAGS))  # 왼쪽부터 CPU
    cell = W // (len(order) + 1)
    lab = font(regular, 24)
    for i, tag in enumerate(order):
        big = icon(tag, 8)
        cx = cell * (i + 1)
        img.paste(big, (cx - big.width // 2, 400))
        text = c["labels"][tag]
        w = d.textlength(text, font=lab)
        d.text((cx - w / 2, 560), text, font=lab, fill=FG)
    center_text(d, 760, c["foot2"], font(regular, 26), MUTED)
    return img


def wallpaper():
    """Windows 11 Bloom을 떠올리게 하는 어두운 배경. 작게 그려 흐린 뒤 키운다."""
    small = Image.new("RGB", (W // 8, H // 8), (10, 13, 28))
    d = ImageDraw.Draw(small)
    for (cx, cy, r, color) in [
        (175, 150, 95, (36, 74, 168)),
        (205, 95, 60, (88, 64, 186)),
        (120, 175, 70, (22, 58, 120)),
        (235, 165, 50, (60, 110, 210)),
    ]:
        d.ellipse([cx - r, cy - r, cx + r, cy + r], fill=color)
    small = small.filter(ImageFilter.GaussianBlur(28))
    return small.resize((W, H), Image.BICUBIC)


class Capture:
    """작업 표시줄 캡처에서 배경을 떼어 낸 전경을 다른 배경 위로 옮긴다.

    작업 표시줄은 아크릴이라 배경색이 열마다 조금씩 다르다. 열마다 빈 행의 중앙값을
    배경으로 보고, 픽셀과 그 배경의 차이를 새 배경에 더한다. 어두운 배경끼리라 글자의
    안티앨리어싱도 거의 그대로 옮겨진다.
    """

    def __init__(self):
        self.img = Image.open(os.path.join(OUT, "screen.png")).convert("RGB")
        self.px = self.img.load()
        y0, y1 = TASKBAR["bg_rows"]
        self.bg = [
            tuple(int(statistics.median(self.px[x, y][c] for y in range(y0, y1))) for c in range(3))
            for x in range(self.img.width)
        ]

    def overlay(self, img, src, at, gate=6):
        """캡처의 `src` 영역(x0, y0, x1, y1) 전경을 `img`의 `at` 위치에 더한다.

        아크릴의 잡음(±3 안팎)이 새 배경에 무늬로 남지 않도록 차이가 `gate` 이하면 버린다.
        트레이 아이콘은 1비트 글꼴이라 `gate`를 크게 잡아도 글자가 깎이지 않는다.
        """
        x0, y0, x1, y1 = src
        ax, ay = at
        dst = img.load()
        for x in range(x0, x1):
            bg = self.bg[x]
            for y in range(y0, y1):
                p = self.px[x, y]
                delta = [p[c] - bg[c] for c in range(3)]
                if max(abs(v) for v in delta) <= gate:
                    continue
                q = dst[ax + x - x0, ay + y - y0]
                dst[ax + x - x0, ay + y - y0] = tuple(
                    max(0, min(255, q[c] + delta[c])) for c in range(3)
                )


def taskbar_shot(lang, c, cap):
    img = wallpaper()
    d = ImageDraw.Draw(img)
    regular, bold = c["font"]
    t = TASKBAR
    bar_h = cap.img.height - t["top"]
    top = H - bar_h

    # 작업 표시줄: 어두운 아크릴처럼 배경을 조금 비친다.
    behind = img.crop((0, top, W, H)).filter(ImageFilter.GaussianBlur(30))
    bar = Image.blend(behind, Image.new("RGB", (W, bar_h), (30, 30, 32)), 0.86)
    img.paste(bar, (0, top))
    base = bar.resize((1, 1), Image.BOX).getpixel((0, 0))
    d.rectangle([0, top, W, top + t["border"] - 1], fill=tuple(v + 26 for v in base))

    body = t["top"] + t["border"]
    y_body = top + t["border"]
    # 왼쪽 정렬 앱
    cell = t["app_cell"]
    x = 6
    for cx in t["apps"]:
        cap.overlay(img, (cx - cell // 2, body, cx + cell // 2, cap.img.height), (x, y_body))
        x += cell

    # 트레이: 오른쪽 끝에서부터 시스템 아이콘, 그 왼쪽에 펼침 단추와 지표 8개
    sx0, sx1 = t["system"]
    tx0, tx1 = t["tray"]
    system_x = W - 36 - (sx1 - sx0)
    tray_x = system_x - t["gap"] - (tx1 - tx0)
    cap.overlay(img, (sx0, body, sx1, cap.img.height), (system_x, y_body))
    cap.overlay(img, (tx0, body, tx1, cap.img.height), (tray_x, y_body))

    # 확대 패널: 트레이 아이콘을 다시 정수 배로 키우고 이름을 붙인다.
    scale = 3
    ry0, ry1 = t["icon_rows"]
    pitch = 196
    pad = 36
    order = list(reversed(TAGS))  # 왼쪽부터 CPU
    panel_w = pitch * len(order) + pad * 2
    panel_h = 270
    first, last = t["icons"][0][0], t["icons"][-1][1]
    icons_mid = tray_x + (first + last) // 2 - tx0
    px0 = max(60, min(W - 60 - panel_w, icons_mid - panel_w // 2))
    py0 = top - 90 - panel_h
    panel_bg = (32, 32, 36)

    # 패널과 실제 트레이를 잇는 쐐기
    lens = Image.new("L", (W, H), 0)
    ImageDraw.Draw(lens).polygon(
        [(px0 + 40, py0 + panel_h), (px0 + panel_w - 40, py0 + panel_h),
         (tray_x + last - tx0 + 10, y_body + 8), (tray_x + first - tx0 - 10, y_body + 8)],
        fill=46,
    )
    img.paste(Image.new("RGB", (W, H), (255, 255, 255)), (0, 0), lens)
    ring = (tray_x + first - tx0 - 14, y_body + 18, tray_x + last - tx0 + 14, H - 18)
    d.rounded_rectangle(ring, radius=12, outline=(150, 175, 230), width=2)

    shadow = Image.new("L", (W, H), 0)
    ImageDraw.Draw(shadow).rounded_rectangle(
        [px0, py0 + 12, px0 + panel_w, py0 + panel_h + 12], radius=24, fill=150)
    img.paste((0, 0, 0), (0, 0), shadow.filter(ImageFilter.GaussianBlur(24)))
    d.rounded_rectangle([px0, py0, px0 + panel_w, py0 + panel_h], radius=24,
                        fill=panel_bg, outline=(70, 70, 76), width=2)

    lab = font(regular, 24)
    for i, tag in enumerate(order):
        x0, x1 = t["icons"][i]
        icon_img = Image.new("RGB", (x1 - x0, ry1 - ry0), panel_bg)
        cap.overlay(icon_img, (x0, ry0, x1, ry1), (0, 0), gate=24)
        big = icon_img.resize((icon_img.width * scale, icon_img.height * scale), Image.NEAREST)
        cx = px0 + pad + pitch * i + pitch // 2
        img.paste(big, (cx - big.width // 2, py0 + 44))
        text = c["labels"][tag]
        w = d.textlength(text, font=lab)
        d.text((cx - w / 2, py0 + 44 + big.height + 40), text, font=lab, fill=FG)

    # 헤드라인
    x, y = 120, 120
    d.text((x, y), c["brand"], font=font(regular, 24), fill=(190, 200, 225))
    y += 54
    head = font(bold, 72)
    for line in c["hero"]:
        d.text((x, y), line, font=head, fill=FG)
        y += 94
    d.text((x, y + 20), c["hero_sub"], font=font(regular, 32), fill=(200, 208, 225))
    return img


def main():
    # 입력이 없는 장은 건너뛴다. 작업 표시줄만 다시 찍었을 때 나머지를 건드리지 않기 위해서다.
    langs = sys.argv[1:] or list(COPY)
    has = lambda name: os.path.exists(os.path.join(OUT, name))  # noqa: E731
    cap = Capture() if has("screen.png") else None
    dumps = all(has(f"dark_px16_{tag}@16x.bmp") for tag in TAGS)
    for lang in langs:
        c = COPY[lang]
        target = os.path.join(DEST, lang)
        os.makedirs(target, exist_ok=True)
        shots = [
            ("01-taskbar.png", cap is not None, lambda: taskbar_shot(lang, c, cap)),
            ("02-settings.png", dumps and has(f"shot-{lang}.png"), lambda: settings_shot(lang, c)),
            ("03-tray.png", dumps, lambda: tray_shot(lang, c)),
        ]
        for name, ready, make in shots:
            if ready:
                make().save(os.path.join(target, name), optimize=True)
                print(f"{lang}: {name}")
            else:
                print(f"{lang}: {name} skipped (input missing in out/)")


if __name__ == "__main__":
    main()
