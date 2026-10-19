"""tray-monitor 의 트레이 아이콘을 트레이에 고정(승격)시킨다.

Windows는 새 트레이 아이콘을 오버플로우(^)에 숨긴다. 앱의 첫 실행 안내가
사용자에게 이 작업을 요청하지만, 개발 중 화면 확인을 위해 스크립트로도 할 수 있다.

사용: python tools/promote.py [실행파일경로]
      경로를 주지 않으면 release 빌드를 대상으로 한다.
"""

import os
import subprocess
import sys

BS = chr(92)
ROOT = "HKEY_CURRENT_USER" + BS + "Control Panel" + BS + "NotifyIconSettings"
DEFAULT_EXE = os.path.join(
    os.path.dirname(os.path.dirname(os.path.abspath(__file__))),
    "target",
    "release",
    "tray-monitor.exe",
)


def keys_for(exe):
    """ExecutablePath 가 exe 인 아이콘 키와 현재 승격 상태."""
    out = subprocess.run(
        ["reg", "query", ROOT, "/s"],
        capture_output=True,
        text=True,
        encoding="utf-8",
        errors="replace",
    ).stdout

    blocks, current = {}, None
    for line in out.splitlines():
        if line.startswith(ROOT):
            rest = line[len(ROOT):]
            if rest:
                current = rest.lstrip(BS)
                blocks[current] = {}
            continue
        parts = line.strip().split(None, 2)
        if len(parts) >= 2 and current:
            blocks[current][parts[0]] = parts[-1].strip()

    found = []
    for key, values in blocks.items():
        if values.get("ExecutablePath", "").lower() == exe.lower():
            found.append((key, values.get("InitialTooltip", "?"), values.get("IsPromoted", "unset")))
    return sorted(found, key=lambda item: item[1])


def promote(exe):
    found = keys_for(exe)
    if not found:
        print(f"{exe} 로 등록된 아이콘 키가 없다. 앱을 한 번 실행해야 한다.")
        return 1
    for key, tooltip, _ in found:
        path = ROOT + BS + key
        subprocess.run(["reg", "add", path, "/v", "IsPromoted", "/t", "REG_DWORD", "/d", "1", "/f"],
                       capture_output=True, text=True)
        print(f"  승격: {tooltip:<10} {key}")
    print(f"{len(found)}개 승격 완료. 트레이에 나타나려면 앱을 다시 시작한다.")
    return 0


if __name__ == "__main__":
    exe = sys.argv[1] if len(sys.argv) > 1 else DEFAULT_EXE
    if not os.path.exists(exe):
        raise SystemExit(f"없는 파일: {exe}")
    print(f"대상: {exe}")
    for key, tooltip, promoted in keys_for(exe):
        print(f"  현재: {tooltip:<10} promoted={promoted}")
    raise SystemExit(promote(exe))
