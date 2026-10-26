"""Store 스크린샷(1920x1080)을 언어별로 합성한다.

입력은 모두 실제 앱과 Windows가 만든 픽셀이다.
- `out/screen.png`: 지표 8개를 고정한 작업 표시줄이 보이는 전체 화면을 225% 배율에서
  `tools/capture_screen.ps1`로 캡처한 것. 아래 `TASKBAR`의 좌표는 이 캡처에서 잰 값이다.
- `out/shot-<lang>.png`: 해당 언어의 설정 창을 `tools/capture_settings.ps1`로 1:1 캡처한 것.

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
        "settings_head": ["Set it up", "your way"],
        "settings_sub": "Every option in one small native window",
        "bullets": [
            "Turn each of the eight readouts on or off",
            "Pick the GPU, network adapter and drive",
            "Colour per readout or taskbar theme",
            "Light, dark or automatic window",
            "No account, no telemetry, no network access",
        ],
        "tray_head": "Eight readouts, one icon each",
        "tray_sub": "Each one lives in its own tray icon. Turn off the ones you do not need.",
        "labels": {
            "CPU": "CPU", "MEM": "Memory", "NETD": "Download", "NETU": "Upload",
            "GPU": "GPU", "VRM": "VRAM", "DSK": "Disk usage", "DIO": "Disk activity",
        },
        "desc": {
            "CPU": "Processor load in %", "MEM": "Memory in use in %",
            "NETD": "Download speed", "NETU": "Upload speed",
            "GPU": "Graphics load in %", "VRM": "Video memory in use in %",
            "DSK": "Drive space used in %", "DIO": "Drive active time in %",
        },
    },
    "ko": {
        "font": ("malgun.ttf", "malgunbd.ttf"),
        "brand": "TINYTRAYMONITOR",
        "hero": ["작업 표시줄에서", "바로 보는 PC 상태"],
        "hero_sub": "CPU, 메모리, 네트워크, GPU, 디스크를 트레이 아이콘 8개로",
        "settings_head": ["원하는 대로", "설정하세요"],
        "settings_sub": "작고 가벼운 네이티브 설정 창 하나로",
        "bullets": [
            "지표 8개를 하나씩 켜고 끄기",
            "측정할 GPU, 네트워크 어댑터, 드라이브 선택",
            "지표별 색상 또는 작업 표시줄 테마",
            "밝게, 어둡게, 자동 창 테마",
            "계정, 원격 분석, 네트워크 접속 없음",
        ],
        "tray_head": "지표 8개, 아이콘도 8개",
        "tray_sub": "지표마다 트레이 아이콘이 따로 있습니다. 필요 없는 지표는 끌 수 있습니다.",
        "labels": {
            "CPU": "CPU", "MEM": "메모리", "NETD": "다운로드", "NETU": "업로드",
            "GPU": "GPU", "VRM": "VRAM", "DSK": "디스크 사용량", "DIO": "디스크 로드율",
        },
        "desc": {
            "CPU": "프로세서 사용률 (%)", "MEM": "메모리 사용률 (%)",
            "NETD": "다운로드 속도", "NETU": "업로드 속도",
            "GPU": "그래픽 사용률 (%)", "VRM": "비디오 메모리 사용률 (%)",
            "DSK": "드라이브 공간 사용률 (%)", "DIO": "드라이브 활성 시간 (%)",
        },
    },
    "ja": {
        "font": ("YuGothM.ttc", "YuGothB.ttc"),
        "brand": "TINYTRAYMONITOR",
        "hero": ["PC の状態を", "タスクバーでひと目で"],
        "hero_sub": "CPU、メモリ、ネットワーク、GPU、ディスクを 8 つのトレイ アイコンに",
        "settings_head": ["好みに合わせて", "設定できます"],
        "settings_sub": "小さなネイティブの設定ウィンドウひとつで",
        "bullets": [
            "8 つの項目を個別にオン・オフ",
            "計測する GPU、アダプター、ドライブを選択",
            "項目ごとの色、またはタスクバーのテーマ",
            "ライト、ダーク、自動のウィンドウ",
            "アカウント不要、テレメトリなし、通信なし",
        ],
        "tray_head": "8 つの項目を 8 つのアイコンで",
        "tray_sub": "項目ごとに別々のトレイ アイコンです。不要な項目はオフにできます。",
        "labels": {
            "CPU": "CPU", "MEM": "メモリ", "NETD": "ダウンロード", "NETU": "アップロード",
            "GPU": "GPU", "VRM": "VRAM", "DSK": "ディスク使用率", "DIO": "ディスク負荷",
        },
        "desc": {
            "CPU": "プロセッサの使用率 (%)", "MEM": "メモリの使用率 (%)",
            "NETD": "ダウンロード速度", "NETU": "アップロード速度",
            "GPU": "グラフィックスの使用率 (%)", "VRM": "ビデオ メモリの使用率 (%)",
            "DSK": "ドライブ容量の使用率 (%)", "DIO": "ドライブのアクティブ時間 (%)",
        },
    },
}


def font(name, size):
    return ImageFont.truetype(os.path.join(FONTS, name), size)


# 배경 블롭: (x, y, 반지름, 색), 1/8 크기 좌표. 장마다 배치를 바꿔 같은 계열로 보이게 한다.
BLOBS = {
    "taskbar": [(175, 150, 95, (36, 74, 168)), (205, 95, 60, (88, 64, 186)),
                (120, 175, 70, (22, 58, 120)), (235, 165, 50, (60, 110, 210))],
    "settings": [(170, 60, 90, (36, 74, 168)), (210, 120, 70, (88, 64, 186)),
                 (110, 140, 60, (22, 58, 120)), (60, 20, 40, (40, 52, 120))],
    "tray": [(40, 150, 90, (36, 74, 168)), (230, 30, 70, (88, 64, 186)),
             (130, 170, 60, (22, 58, 120)), (200, 150, 50, (60, 90, 200))],
}


def wallpaper(kind="taskbar"):
    """Windows 11 Bloom을 떠올리게 하는 어두운 배경. 작게 그려 흐린 뒤 키운다."""
    small = Image.new("RGB", (W // 8, H // 8), (10, 13, 28))
    d = ImageDraw.Draw(small)
    for (cx, cy, r, color) in BLOBS[kind]:
        d.ellipse([cx - r, cy - r, cx + r, cy + r], fill=color)
    small = small.filter(ImageFilter.GaussianBlur(28))
    return small.resize((W, H), Image.BICUBIC)


def shadow(img, box, radius, blur=30, offset=14, strength=170):
    """`box` 아래에 부드러운 그림자를 깐다."""
    mask = Image.new("L", (W, H), 0)
    x0, y0, x1, y1 = box
    ImageDraw.Draw(mask).rounded_rectangle([x0, y0 + offset, x1, y1 + offset], radius=radius,
                                           fill=strength)
    img.paste((0, 0, 0), (0, 0), mask.filter(ImageFilter.GaussianBlur(blur)))


def glass(img, box, radius, tint=(26, 28, 40), alpha=0.72, outline=(80, 84, 104)):
    """배경을 흐리고 어둡게 덮은 둥근 카드(아크릴 느낌)."""
    x0, y0, x1, y1 = box
    region = img.crop(box).filter(ImageFilter.GaussianBlur(24))
    region = Image.blend(region, Image.new("RGB", region.size, tint), alpha)
    mask = Image.new("L", region.size, 0)
    ImageDraw.Draw(mask).rounded_rectangle([0, 0, x1 - x0 - 1, y1 - y0 - 1], radius=radius, fill=255)
    img.paste(region, (x0, y0), mask)
    ImageDraw.Draw(img).rounded_rectangle(box, radius=radius, outline=outline, width=1)


def headline(d, c, x, y, lines, sub, size=72):
    """브랜드, 두 줄 헤드라인, 부제를 그리고 다음 y를 돌려준다."""
    regular, bold = c["font"]
    d.text((x, y), c["brand"], font=font(regular, 24), fill=(190, 200, 225))
    y += 54
    head = font(bold, size)
    for line in lines:
        d.text((x, y), line, font=head, fill=FG)
        y += int(size * 1.3)
    d.text((x, y + 16), sub, font=font(regular, 30), fill=(200, 208, 225))
    return y + 16 + 50


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

    def foreground(self, src, gate=24):
        """`src` 영역에서 배경과 `gate`보다 다른 픽셀만 원래 색 그대로 남긴 RGBA 조각.

        1비트 글꼴 아이콘처럼 안티앨리어싱이 없는 전경에만 쓴다.
        """
        x0, y0, x1, y1 = src
        out = Image.new("RGBA", (x1 - x0, y1 - y0), (0, 0, 0, 0))
        dst = out.load()
        for x in range(x0, x1):
            bg = self.bg[x]
            for y in range(y0, y1):
                p = self.px[x, y]
                if max(abs(p[c] - bg[c]) for c in range(3)) > gate:
                    dst[x - x0, y - y0] = (*p, 255)
        return out

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


def settings_shot(lang, c):
    img = wallpaper("settings")
    d = ImageDraw.Draw(img)
    regular, _ = c["font"]

    y = headline(d, c, 120, 150, c["settings_head"], c["settings_sub"])
    # 기능 목록: 강조색 점과 함께
    body = font(regular, 30)
    y += 40
    accents = [(108, 214, 120), (110, 160, 255), (255, 170, 80), (190, 140, 255), (120, 220, 230)]
    for bullet, accent in zip(c["bullets"], accents):
        d.ellipse([120, y + 14, 132, y + 26], fill=accent)
        d.text((152, y), bullet, font=body, fill=(230, 234, 245))
        y += 58

    # 설정 창: 1:1 캡처에 Windows 11처럼 둥근 모서리와 그림자를 준다.
    dialog = Image.open(os.path.join(OUT, f"shot-{lang}.png")).convert("RGB")
    dw, dh = dialog.size
    dx, dy = W - 140 - dw, (H - dh) // 2
    glow = Image.new("L", (W, H), 0)
    ImageDraw.Draw(glow).rounded_rectangle([dx - 60, dy - 60, dx + dw + 60, dy + dh + 60],
                                           radius=80, fill=90)
    img.paste((70, 90, 200), (0, 0), glow.filter(ImageFilter.GaussianBlur(70)))
    shadow(img, (dx, dy, dx + dw, dy + dh), radius=8, blur=34, offset=18, strength=200)
    mask = Image.new("L", dialog.size, 0)
    ImageDraw.Draw(mask).rounded_rectangle([0, 0, dw - 1, dh - 1], radius=8, fill=255)
    img.paste(dialog, (dx, dy), mask)
    d.rounded_rectangle([dx, dy, dx + dw - 1, dy + dh - 1], radius=8, outline=(84, 84, 90), width=1)
    return img


def tray_shot(lang, c, cap):
    img = wallpaper("tray")
    d = ImageDraw.Draw(img)
    regular, bold = c["font"]
    t = TASKBAR

    x0 = 120
    d.text((x0, 110), c["brand"], font=font(regular, 24), fill=(190, 200, 225))
    d.text((x0, 160), c["tray_head"], font=font(bold, 64), fill=FG)
    d.text((x0, 262), c["tray_sub"], font=font(regular, 30), fill=(200, 208, 225))

    # 4x2 카드. 아이콘은 225% 캡처(16px 원본의 2배)를 다시 3배로 키운다.
    order = list(reversed(TAGS))  # 왼쪽 위부터 CPU
    cols, gap = 4, 28
    card_w = (W - x0 * 2 - gap * (cols - 1)) // cols
    card_h = 300
    top = 370
    ry0, ry1 = t["icon_rows"]
    name_font, desc_font = font(bold, 32), font(regular, 24)
    for i, tag in enumerate(order):
        r, k = divmod(i, cols)
        bx = x0 + k * (card_w + gap)
        by = top + r * (card_h + gap)
        box = (bx, by, bx + card_w, by + card_h)
        shadow(img, box, radius=22, blur=20, offset=10, strength=120)
        glass(img, box, radius=22)
        sx0, sx1 = t["icons"][i]
        fg = cap.foreground((sx0 - 4, ry0, sx1 + 4, ry1))
        big = fg.resize((fg.width * 3, fg.height * 3), Image.NEAREST)
        img.paste(big, (bx + 40, by + 40), big)
        d.text((bx + 40, by + card_h - 112), c["labels"][tag], font=name_font, fill=FG)
        d.text((bx + 40, by + card_h - 64), c["desc"][tag], font=desc_font, fill=(185, 192, 212))
    return img


def main():
    # 입력이 없는 장은 건너뛴다. 작업 표시줄만 다시 찍었을 때 나머지를 건드리지 않기 위해서다.
    langs = sys.argv[1:] or list(COPY)
    has = lambda name: os.path.exists(os.path.join(OUT, name))  # noqa: E731
    cap = Capture() if has("screen.png") else None
    for lang in langs:
        c = COPY[lang]
        target = os.path.join(DEST, lang)
        os.makedirs(target, exist_ok=True)
        shots = [
            ("01-taskbar.png", cap is not None, lambda: taskbar_shot(lang, c, cap)),
            ("02-settings.png", has(f"shot-{lang}.png"), lambda: settings_shot(lang, c)),
            ("03-tray.png", cap is not None, lambda: tray_shot(lang, c, cap)),
        ]
        for name, ready, make in shots:
            if ready:
                make().save(os.path.join(target, name), optimize=True)
                print(f"{lang}: {name}")
            else:
                print(f"{lang}: {name} skipped (input missing in out/)")


if __name__ == "__main__":
    main()
