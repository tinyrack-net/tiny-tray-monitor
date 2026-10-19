"""HKCU NotifyIconSettings 를 파싱해 tray-monitor 의 아이콘 키를 출력한다.

아이콘 식별 키가 (실행 경로 + 등록 시점 툴팁) 에 묶이는지 확인하는 실험용 도구.
"""

import re
import subprocess
import sys

BS = chr(92)  # backslash
ROOT = "HKEY_CURRENT_USER" + BS + "Control Panel" + BS + "NotifyIconSettings"
NIS = "NotifyIconSettings" + BS


def query():
    out = subprocess.run(
        ["reg", "query", ROOT, "/s"],
        capture_output=True,
        text=True,
        encoding="utf-8",
        errors="replace",
    ).stdout

    blocks = {}
    cur = None
    for line in out.splitlines():
        if line.startswith(ROOT):
            rest = line[len(ROOT):]
            if not rest:
                continue
            cur = rest.lstrip(BS)
            blocks[cur] = {}
            continue
        m = re.match(r"^\s+(\w+)\s+REG_\w+\s+(.*)$", line)
        if m and cur:
            blocks[cur][m.group(1)] = m.group(2).strip()

    rows = []
    for key, v in blocks.items():
        if "tray-monitor" in v.get("ExecutablePath", "").lower():
            rows.append(
                (
                    v.get("InitialTooltip", "<none>"),
                    key,
                    v.get("IsPromoted", "unset"),
                )
            )
    return sorted(rows)


if __name__ == "__main__":
    rows = query()
    print(f"{len(rows)} icon keys for tray-monitor")
    for tip, key, promoted in rows:
        print(f"  promoted={promoted:<8} tooltip={tip!r:<18} key={key}")
    if len(sys.argv) > 1:
        with open(sys.argv[1], "w", encoding="utf-8") as f:
            f.write("\n".join(f"{t}|{k}" for t, k, _ in rows))
