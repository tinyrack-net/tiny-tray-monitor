"""스파이크가 등록한 NotifyIconSettings 키를 삭제한다.

주의: 지금 실행 중인 앱의 키도 같이 지운다. 키를 지우면 사용자가 고정해 둔
트레이 승격도 사라지고, 그 Explorer 세션에서는 키가 다시 기록되지 않는다
(docs/SPIKE.md §14). 그래서 기본은 **미리보기**이고, 실제 삭제에는 --force가 필요하다.

    python tools/traykeys_cleanup.py            # 무엇이 지워질지만 출력
    python tools/traykeys_cleanup.py --force     # 실제로 삭제
"""

import subprocess
import sys

from traykeys import query

ROOT = "HKCU" + chr(92) + "Control Panel" + chr(92) + "NotifyIconSettings"

force = "--force" in sys.argv[1:]
rows = query()
print(f"{'삭제 대상' if force else '삭제 예정(미리보기)'} {len(rows)}개")
for tip, key, promoted, guid, _ in rows:
    label = f"{tip!r} guid={guid}"
    if not force:
        print(f"  {label:<60} {key}")
        continue
    path = ROOT + chr(92) + key
    r = subprocess.run(["reg", "delete", path, "/f"], capture_output=True, text=True)
    result = "삭제됨" if r.returncode == 0 else "실패: " + r.stderr.strip()
    print(f"  {label:<60} {key} -> {result}")
if not force and rows:
    print("실제로 지우려면 --force를 붙인다. 실행 중인 앱의 키를 지우면 트레이 고정이 풀린다.")
