"""Store 스크린샷(1920x1080)을 언어별로 합성한다.

입력은 모두 실제 앱이 만든 픽셀이다.
- `out/shot-<lang>.png`: 해당 언어의 설정 창을 PrintWindow로 1:1 캡처한 것
  (`resources/store/README.md`의 절차).
- `out/dark_px16_<TAG>@16x.bmp`: `tiny-tray-monitor --dump`가 그린 16px 트레이 아이콘.

결과는 `resources/store/screenshots/<lang>/01-settings.png`, `02-tray.png`.

필요: Pillow (`pip install pillow`). 사용: python tools/store_shots.py
"""

import os
import sys

from PIL import Image, ImageDraw, ImageFont

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

COPY = {
    "en": {
        "font": ("segoeui.ttf", "segoeuib.ttf"),
        "brand": "TINYTRAYMONITOR",
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


def main():
    langs = sys.argv[1:] or list(COPY)
    for lang in langs:
        c = COPY[lang]
        target = os.path.join(DEST, lang)
        os.makedirs(target, exist_ok=True)
        settings_shot(lang, c).save(os.path.join(target, "01-settings.png"), optimize=True)
        tray_shot(lang, c).save(os.path.join(target, "02-tray.png"), optimize=True)
        print(f"{lang}: {target}")


if __name__ == "__main__":
    main()
