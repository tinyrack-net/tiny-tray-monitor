"""스파이크가 등록한 NotifyIconSettings 키를 삭제한다."""
import subprocess
from traykeys import query

rows = query()
print(f"삭제 대상 {len(rows)}개")
for tip, key, promoted in rows:
    path = "HKCU" + chr(92) + "Control Panel" + chr(92) + "NotifyIconSettings" + chr(92) + key
    r = subprocess.run(["reg", "delete", path, "/f"], capture_output=True, text=True)
    print(f"  {tip!r:<12} {key} -> {'삭제됨' if r.returncode == 0 else '실패: ' + r.stderr.strip()}")
