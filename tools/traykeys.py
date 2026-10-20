"""HKCU NotifyIconSettings 를 파싱해 tray-monitor 의 아이콘 키를 출력한다.

진단용 도구. 아이콘 정체성은 `(ExecutablePath, UID)` 로 확인됐고, `InitialTooltip` 은
키가 아니라 최초 등록 시점에 기록되는 값이다. GUID로 등록한 아이콘은 `IconGuid` 가
정체성이고 `UID` 는 기록되지 않는다. 실측 근거는 docs/SPIKE.md §14.
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
                    v.get("IconGuid", "-"),
                    v.get("UID", "-"),
                )
            )
    return sorted(rows)


if __name__ == "__main__":
    rows = query()
    print(f"{len(rows)} icon keys for tray-monitor")
    for tip, key, promoted, guid, uid in rows:
        print(
            f"  promoted={promoted:<8} uid={uid:<6} tooltip={tip!r:<20} "
            f"guid={guid} key={key}"
        )
    if len(sys.argv) > 1:
        with open(sys.argv[1], "w", encoding="utf-8") as f:
            f.write("\n".join(f"{t}|{k}" for t, k, _ in rows))
