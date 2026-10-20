# tray-monitor 구현 계획

Windows 트레이에서 CPU / 메모리 / 네트워크 상태를 실시간으로 보여주는 시스템 모니터.

- 저장소: `~/Workspaces/tinyrack/tray-monitor` → `github.com/tinyrack-net/tray-monitor`
- 라이선스: MIT (`Copyright (c) 2026 winetree94`, 조직 표준)
- 검증 근거: [SPIKE.md](SPIKE.md) — 트레이 텍스트 렌더링, 가독성 한계, 승격 동작을 실측한 기록

---

## 1. 확정된 설계 결정

| 항목 | 결정 | 근거 |
| --- | --- | --- |
| 스택 | Rust + `tray-icon` + `windows-rs` | 상주 10.6MB / 코어 0.15%. 단일 exe |
| 지표 | CPU, 메모리, 네트워크 | 관리자 권한·커널 드라이버 불필요 |
| 표시 | 아이콘 3개, **라벨 + 값 2줄** | 타입 식별을 위해 가독성을 일부 양보 |
| 렌더 | GDI DIB + 8배 슈퍼샘플 + bbox 크롭 + 박스 다운샘플 | 외부 이미지 크레이트 없이 AA 품질 확보 |
| 값 캐시 | 표시 문자열 단위 캐시 | 값이 유한 집합이라 히트율이 높다 (코어 0.32→0.15%) |
| 플랫폼 | Windows 전용 | 지표 수집과 렌더가 모두 Win32/GDI |

### 아이콘 레이아웃 (16px 기준 실측)

```
┌────────┐   label_h = 5px   라벨(CPU/MEM/NET), 밝기 190/255 로 뒤로 물림
│  CPU   │   gap      = 1px
│  80%   │   value_h  = 9px   값, 가로 압축(Squeeze)으로 높이를 살림
└────────┘
```

측정 결과 (자세한 표는 [SPIKE.md](SPIKE.md#2-정사각형-제약이-설계를-지배한다)):

| 레이아웃 | 라벨 높이 | 값 높이 | 판정 |
| --- | --- | --- | --- |
| 값만 (라벨 없음) | - | 6px | 값이 작다 |
| **라벨 5 + 값 9(압축)** | 5px | 9px | **채택** |
| 라벨 4 + 값 11(압축) | 4px | 11px | 라벨이 읽히지 않음 |
| 라벨 6 + 값 8(압축) | 6px | 8px | 라벨은 낫지만 값이 작아짐 |

### 값 표기 규격

정사각형 아이콘이라 값을 3~4자로 제한한다. 4자는 가로 압축이 심해지므로 예외로 둔다.

| 지표 | 라벨 | 값 예 | 규칙 |
| --- | --- | --- | --- |
| CPU | `CPU` | `80%` | 0~100, `%` 포함 최대 4자 |
| MEM | `MEM` | `77%` | 동일 |
| NET | `NET` | `24M` | 다운로드 속도. `999K`/`24M`/`1.2G` 처럼 3~4자 |

업로드 속도, 정확한 수치, 인터페이스별 내역은 **툴팁**에 넣는다.
아이콘 px가 24 이상(배율 150% 이상)이면 값 문자 수를 한 단계 늘린다(`80%` → 그대로, NET은 `1.2M`).

---

## 2. 저장소 구조

```
tray-monitor/
├── Cargo.toml                # edition 2024, release: lto / opt-level=z / strip
├── LICENSE                   # MIT
├── README.md                 # 조직 표준(영문 본문 + 한국어)
├── AGENTS.md                 # 프로젝트 개요 / 검증 루프 / 구조 / 병합 / 릴리스
├── Makefile                  # make validate / run / icon-sheet
├── rust-toolchain.toml
├── .github/workflows/pipeline.yml
├── docs/
│   ├── PLAN.md               # 이 문서
│   ├── SPIKE.md              # 실측 검증 기록
│   └── design/
│       ├── icon.md           # 렌더 규격, 레이아웃, 색, 캐시
│       └── metrics.md        # 지표 수집 규격과 포맷 규칙
├── src/
│   ├── main.rs               # 부트스트랩: DPI, 단일 인스턴스, 트레이 생성, 타이머, 메시지 루프
│   ├── app.rs                # 상태와 갱신 오케스트레이션
│   ├── metrics/
│   │   ├── mod.rs            # Sampler trait, 샘플 값 타입
│   │   ├── cpu.rs            # GetSystemTimes 델타
│   │   ├── memory.rs         # GlobalMemoryStatusEx
│   │   ├── network.rs        # GetIfTable2 합계 (루프백/터널 제외)
│   │   └── format.rs         # 값 -> 표시 문자열 (순수 함수, 테스트 대상)
│   ├── icon/
│   │   ├── gdi.rs            # DIB/폰트/DrawText 래퍼 (unsafe 격리)
│   │   ├── layout.rs         # px별 레이아웃 정책 (순수)
│   │   ├── render.rs         # 슈퍼샘플 -> 잉크 bbox -> 다운샘플
│   │   └── cache.rs          # 표시 문자열 캐시
│   ├── tray/
│   │   ├── mod.rs            # 슬롯 관리, 우클릭 메뉴, 승격 감지
│   │   └── onboard.rs        # 첫 실행 안내
│   ├── config.rs             # 설정 스키마, 로드/저장
│   ├── startup.rs            # HKCU Run 키 등록/해제
│   ├── theme.rs              # 작업 표시줄 테마 -> 글자색
│   └── win32.rs              # 공용 Win32 헬퍼
└── tools/                    # 스파이크에서 이관한 계측 도구
    ├── icon_sheet.py         # 레이아웃 비교 시트 생성
    ├── crop_png.py           # 트레이 스크린샷 크롭
    └── traykeys.py           # 트레이 승격 상태 조회
```

**경계 원칙**

- `metrics/`와 `icon/`은 트레이를 모른다. 트레이는 바이트 배열과 문자열만 받는다.
- `unsafe`는 `gdi.rs`와 `win32.rs`에만 둔다. 나머지는 안전한 Rust.
- 델타 계산·포맷·레이아웃 선택은 **순수 함수**로 떼어내 단위 테스트한다.

---

## 3. 마일스톤

각 단계는 독립적으로 동작하는 상태로 끝난다. 예상 합계 8~10일.

### M0. 저장소 골격 (0.5일)

- `gh repo create tinyrack-net/tray-monitor --public --source . --push`
- LICENSE / README / AGENTS.md / .gitignore / rust-toolchain.toml / Makefile
- `pipeline.yml` + **Quality Gate** 집계 잡, `main` 보호 + merge queue
- **완료 조건**: PR 하나가 Quality Gate를 통과해 squash merge 된다

### M1. 지표 수집 계층 (1일)

- `cpu.rs` / `memory.rs` / `network.rs` / `format.rs` 이관, 로그 출력 모드(`--probe`)
- 단위 테스트: 첫 샘플 0 처리, 델타 0 나눗셈, 값 역행/오버플로 클램프, 포맷 경계값
- **완료 조건**: `cargo test` 통과, `--probe`로 10초간 값이 합리적으로 갱신

### M2. 아이콘 렌더 계층 (1.5일)

- `gdi.rs` / `render.rs` / `layout.rs` / `cache.rs` 이관
- `--dump`로 레이아웃 시트 생성, 픽셀 불변식 테스트
- **완료 조건**: 16/24/32px에서 라벨+값이 박스 안에 들어가고 글리프 높이가 규격을 만족

### M3. 트레이 앱 조립 (1일)

- 슬롯 3개, 1초 타이머, 우클릭 메뉴(정보 / 설정 열기 / 종료), 종료 시 아이콘 정리
- 단일 인스턴스(mutex), `windows_subsystem = "windows"`(릴리스에서 콘솔 없음)
- **완료 조건**: 재실행·Explorer 재시작·절전 복귀 후에도 아이콘 3개가 정상 갱신

### M4. 설정 (0.5일)

- `%APPDATA%\tray-monitor\config.toml`: 표시 항목, 순서, 폴링 주기, 색, 자동 시작
- **완료 조건**: 설정 변경이 재시작 후 유지되고 잘못된 값은 거부된다

### M5. 온보딩 + 자동 시작 (1일)

- 첫 실행 시 승격 안내(트레이 아이콘이 숨겨져 있을 수 있음을 알림), `ms-settings:taskbar` 열기
- 승격 상태 감지: `NotifyIconSettings` 하위 키를 `ExecutablePath`로 매칭
- **완료 조건**: 승격 전에는 안내하고, 승격 후에는 안내하지 않는다

### M6. 표시 품질 (1.5일)

- 작업 표시줄 테마(라이트/다크)에 따른 글자색 전환
- 지표별 색, px별 레이아웃 정책, 폰트 폴백
- **완료 조건**: 라이트/다크 양쪽 배경에서 아이콘이 읽힌다

### M7. 릴리스 파이프라인 (1일)

- 태그 `v*.*.*` → 빌드 → 포터블 zip + Inno Setup installer + MSIX + `SHA256SUMS` → `gh release create --generate-notes`
- 세부 설계와 실측은 [PACKAGING.md](PACKAGING.md)
- winget / scoop 매니페스트는 후속 (조직에 `setup_winget` 롤이 있다)
- **완료 조건**: 태그로 릴리스가 자동 생성되고 세 산출물이 모두 실행된다

### M8. 문서 (0.5일)

- README(영문/한국어), 스크린샷, 온보딩 문서, AGENTS.md 최종화

---

## 4. CI / 릴리스 (조직 표준)

`proxer` / `dotweave` / `mac-cjkv-input-switcher` 규칙을 따른다.

- 워크플로는 `.github/workflows/pipeline.yml` 하나. 이름은 `CI/CD Pipeline`
- **`Quality Gate`** 집계 잡이 유일한 필수 체크다. `needs`에 넣은 잡이 `skipped`면 게이트도 실패하므로,
  모든 잡은 `pull_request`와 `merge_group` 양쪽에서 실행되어야 한다
- `main`은 직접 푸시 금지. `gh pr merge --auto --squash` + merge queue
- 품질 잡(모두 Windows): `cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test`, `cargo build --release`
- 러너: `windows-latest` (조직 자체 러너 `tinyrack-windows` 사용 가능)
- 릴리스: `v*.*.*` 태그 → `gh release create "$GITHUB_REF_NAME" ... --generate-notes`

---

## 5. 구현 시 반드시 지킬 제약

스파이크에서 실측으로 확인된 것들이라, 어기면 바로 버그가 된다.

1. **빈 문자열을 GDI에 넘기지 않는다.** 프로세스가 즉시 죽는다. 렌더 진입점에 가드가 있어야 한다.
2. **DIB의 알파 바이트를 읽지 않는다.** GDI는 B, G, R만 채운다. 휘도는 R 채널에서 읽는다.
3. **`SetTimer(hwnd=NULL)`의 반환값으로 타이머를 식별한다.** 요청한 ID는 무시된다.
4. **exe 경로를 바꾸지 않는다.** 트레이 승격 설정은 기본적으로 경로에 묶여 있어, 버전별
   폴더로 업데이트하면 초기화된다. installer 자동 업데이트는 제자리 교체로 설계한다.
   MSIX처럼 경로가 바뀔 수밖에 없는 채널은 지표별 고정 GUID로 승격을 유지한다.
5. **등록 시점의 툴팁은 고정 문자열로 둔다.** 승격 설정은 툴팁 변경에 안전한 것으로 확인했지만,
   등록 시점 문자열은 사용자에게 보이는 최초 식별자이므로 살아있는 값을 넣지 않는다.
6. **1초마다 새 폰트/DC를 만들지 않는다.** 표시 문자열 캐시를 반드시 유지한다 (코어 사용 2배 차이).

---

## 6. 먼저 확인해야 하는 미검증 항목

M1~M3 사이에 실험으로 닫아야 한다. 설계를 바꿀 수 있는 것들이다.

| 항목 | 왜 중요한가 | 확인 방법 |
| --- | --- | --- |
| 라이트 테마에서 아이콘 색 | 작업 표시줄이 밝으면 흰 글자가 안 보인다. 테마 감지가 필수인지 결정 | 밝은 테마로 전환 후 스크린샷 |
| Explorer 재시작 | `tray-icon`이 `TaskbarCreated`를 처리하는지 미확인. 안 하면 아이콘이 사라진다 | 작업 관리자에서 explorer.exe 재시작 |
| DPI 변경 | 아이콘을 다시 그려야 하는지, 셸이 스케일하는지 | 배율 100↔150% 전환 |
| 절전 복귀 | 누적 카운터 델타가 폭주해 CPU 100%로 튈 수 있다 | 절전 후 복귀 로그 확인 |
| 트레이 아이콘 상한 | 3개가 넘어가면 오버플로우가 길어진다 | 사용자 피드백 |

---

## 7. 테스트 전략

| 대상 | 방법 |
| --- | --- |
| 델타 계산, 값 포맷, 레이아웃 선택, 설정 파싱 | 순수 함수 단위 테스트 (`cargo test`) |
| 아이콘 렌더 | **정확한 픽셀 해시 비교는 하지 않는다.** Windows 버전별 AA 차이로 깨진다. 대신 불변식 검사: 잉크가 박스 안에 있다 / 글리프 높이가 기대 범위다 / 빈 입력에서 크래시하지 않는다 |
| 지표 수집 | 실제 API를 호출하는 스모크 테스트(`--probe`)로 대체. CI에서는 값의 범위만 검사 |
| 트레이 | 수동 검증 체크리스트 (docs/design/icon.md에 둔다) |
| 아이콘 가독성 | `tools/icon_sheet.py`로 레이아웃 시트를 만들어 눈으로 판정. 새 레이아웃을 추가할 때마다 갱신 |

`make validate` = `fmt --check` + `clippy -D warnings` + `test` + `build --release`.

---

## 8. 범위 밖 (v1)

- CPU 온도 / 팬 / 전압: 관리자 권한과 커널 드라이버가 필요하다. 별도 관리자 헬퍼 + IPC로 후속 검토
- GPU: PDH `GPU Engine` 카운터로 무권한 수집 가능하나 v1에서 제외
- 디스크 I/O, 배터리, 프로세스 목록
- 과거 이력 그래프: 상세 대시보드 창이 필요하므로 별도 제품 판단
- macOS / Linux: `tray-icon`은 지원하지만 지표 수집과 렌더가 Win32/GDI 전용이다

---

## 9. 열려 있는 결정

1. **제품 이름** — 저장소는 `tray-monitor`로 확정. 표시 이름과 브랜딩은 미정
2. **표시 항목 선택 방식** — 3개 고정인지, 사용자가 항목과 순서를 바꿀 수 있는지 (M4 범위)
3. **설정 UI** — 트레이 우클릭 메뉴로 충분한지, 별도 창이 필요한지 (M4 범위)
4. **자동 업데이트** — GitHub 릴리스 확인 후 알림만 할지, 자동 교체까지 할지 (M7 이후)
5. **아이콘 색** — 지표별 색을 쓸지, 테마별 단색으로 갈지 (M6에서 시트로 비교)

---

## 10. 진행 결과 (M0 ~ M8 완료)

[`v0.1.0`](https://github.com/tinyrack-net/tray-monitor/releases/tag/v0.1.0)으로 릴리스했다.

| 항목 | 결과 |
| --- | --- |
| 상주 메모리 | **18.7 MB** 작업집합 (개인 메모리 2.9 MB, 핸들 216개 고정) |
| CPU 사용 | 1초 주기에서 단일 코어의 **0.41%** (아이콘 3개 × 글리프 2개) |
| 글리프 캐시 재사용률 | 1초 주기 59%, 250ms 주기 79% |
| 바이너리 | 490 KB (release, lto + strip) |
| 테스트 | 91개 (GDI 핸들 누수 검사 포함) |

계획과 달라진 점:

- **글리프가 3개에서 6개로 늘어 CPU 사용이 0.15%에서 0.41%가 됐다.** 계획의 0.15%는
  스파이크에서 아이콘당 1글자를 그린 값이었다. 라벨이 붙으면 2배가 된다. 여전히 무시할 수준이다.
- **메모리는 시작 직후 값(11.4 MB)이 아니라 정상 상태 값(18.7 MB)을 봐야 한다.** 24분 가동 후
  측정하면 18.7 MB에서 더 늘지 않는다. 개인 메모리는 2.9 MB, 핸들 수는 216개로 고정이다.
- **트레이 순서는 셸이 관리한다.** 처음에는 `CPU / MEM / NET` 순으로 등록되지만, 승격되거나
  Explorer가 재시작되면 순서가 달라질 수 있다. 사용자가 드래그로 바꿀 수 있으므로 앱이
  강제하지 않는다.
- **`Metric::ALL` 순서가 곧 아이콘 id다.** Windows는 아이콘 id를 생성 순서로 발급하고
  승격 설정을 그 id에 묶는다. 그래서 꺼진 지표도 등록은 유지하고 `set_visible(false)`로
  숨긴다. 고정 GUID가 없는 경로 기반 등록에서는 실행 경로가 바뀌면 승격이 초기화되므로
  installer 업데이트는 제자리 교체여야 한다. MSIX는 고정 GUID로 경로 변경에도 유지된다.
- **M6에서 DPI 변경은 한계로 남겼다.** 아이콘은 시작 시 배율로 한 번 그린다. 배율을 바꾸면
  작업 표시줄이 기존 비트맵을 늘려 쓴다.

### 시각 검증에서 찾은 결함 (계획에 없던 것)

1. **트레이 메뉴를 열면 아이콘이 멈췄다.** `SetTimer(None, ...)`은 `WM_TIMER`를 스레드 큐로
   보내는데, `TrackPopupMenu`의 모달 루프가 그 메시지를 버린다. 메시지 전용 창을 만들고
   `WndProc`에서 타이머를 받아 해결했다.
2. **첫 실행 안내가 뜨면 프로세스가 조용히 죽었다.** 모달 대화상자가 중첩 메시지 루프를
   돌리면서 타이머가 재진입했고, 콜백을 보관한 `RefCell`을 대여 중이라 패닉이 났다
   (릴리스는 `panic = "abort"`). 콜백을 꺼내서 실행하도록 고치고, 대여 실패를 종료로
   해석하던 `should_exit`도 함께 고쳤다.
3. **대화상자가 다른 창 뒤에 깔렸다.** 소유 창이 없어서다. `MB_SETFOREGROUND | MB_TOPMOST`를
   붙여 항상 앞에 오게 했다.
4. **릴리스 잡이 Windows에서 실패했다.** `Copy-Item`의 위치 인자와 `gh`의 와일드카드 확장
   문제였다. CI에서만 드러나는 종류라 태그를 실제로 찍어 보는 것이 유일한 검증이었다.

### 남은 일

- 설치 프로그램과 MSIX는 v0.4.0에서 추가했다([PACKAGING.md](PACKAGING.md)). winget/scoop 매니페스트는 남아 있다.
- 자동 업데이트: 릴리스 확인 후 알림, 필요하면 제자리 교체
- CPU 온도: 관리자 헬퍼 + IPC로 분리해 옵션으로
- 배율 변경 시 아이콘 재렌더 (승격 설정을 유지한 채 크기만 바꾸는 경로 필요)

---

## 11. 네트워크 다운/업 분리 (v0.2.0)

`NET` 아이콘 하나를 `DN` / `UP` 두 개로 나눴다.

### 라벨 후보 실측

라벨 줄은 16px 아이콘에서 5px다. 후보를 같은 조건으로 렌더해 비교했다
(`make dump` → `tools/icon_sheet.py --files`).

| 후보 | 결과 |
| --- | --- |
| `↓` / `↑` | 5px에서 얇은 세로획으로 뭉개져 **둘이 구분되지 않는다** |
| `DN` / `UP` | 또렷하게 읽힌다. **채택** |
| `RX` / `TX` | 또렷하지만 일반 사용자에게 생소하다 |

색도 분리했다(다운로드 주황 / 업로드 보라). 5px 라벨을 보완하는 두 번째 단서다.

### 아이콘 id 승계

아이콘 id는 `Metric::ALL` 순서로 부여되고 승격 설정이 그 id에 묶인다.
그래서 `NetworkDown`을 세 번째(기존 `NET` 자리)에 두었다. v0.1.0에서 `NET`을 승격해 둔
사용자는 다운로드 아이콘을 계속 보고, 새 `UP` 아이콘만 오버플로우에 숨는다.

승격되지 않은 아이콘이 남아 있으면 버전당 한 번 안내하므로, 업그레이드 후
"3 of 4 readouts are visible" 안내가 뜬다(실측 확인).

MSIX처럼 설치 경로가 버전마다 바뀌는 채널에서는 아이콘 id 대신 지표별 고정 GUID로 등록한다.
Windows가 승격 설정을 GUID에 묶으므로 업데이트 후에도 사용자의 선택이 유지된다. GUID와
`Metric::ALL` 순서는 함께 고정이다.

### 설정 마이그레이션

v0.1.0의 `[show] network = true`를 `network_down` / `network_up`으로 옮기고 legacy 항목은
다시 쓰지 않는다(`#[serde(skip_serializing)]`).
---

## 12. 설정 GUI (v0.3.0)

설정을 바꾸는 길이 지금 둘뿐이다: 트레이 메뉴 토글, 그리고 `config.toml` 직접 편집.
항목이 늘었고(`color_mode`, `exclude_virtual_adapters`, 지표 4종) 파일 편집을 안내하는 것은
일반 사용자에게 요구가 높다. 설정 창 하나로 모은다.

### 12.1 확정된 결정

| 항목 | 결정 | 근거 |
| --- | --- | --- |
| 구현 방식 | **Win32 대화상자 + `.rc` 리소스** (`embed-resource`) | §12.2 |
| 반영 시점 | **즉시 적용 + 닫기** (`OK`/`Cancel` 없음) | 되돌리기 경로가 필요 없고 트레이 메뉴 토글과 동작이 같다. 바꾼 결과를 그 자리에서 본다. 되돌리기 수단은 `Reset to defaults` |
| 범위 | 핵심 설정 5종 + 아이콘 미리보기 + 바로가기·기본값 복원 + 다크 모드 | §12.4 |
| 진입 | 트레이 메뉴 `Settings…`, `--settings`, **exe 재실행 시 설정 창** | 메뉴에서만 열면 exe를 다시 실행하는 사용자를 놓친다 |
| 스레드 | 메인 스레드 **모달** 대화상자 | 앱이 단일 스레드 + 스레드 지역 `App`이다. 별도 스레드는 `App` 공유 모델부터 다시 설계해야 한다 |
| 다크 모드 | 작업 표시줄 테마를 따라간다. 실패하면 라이트로 조용히 폴백 | 이미 `theme::is_light()`가 있다. 비공식 API는 한 모듈에 격리 |

### 12.2 방식 선택 근거

| 후보 | 판단 |
| --- | --- |
| **Win32 대화상자 + `.rc`** | **채택.** 프로세스가 이미 `PER_MONITOR_AWARE_V2`라 대화상자·메시지 박스의 자동 스케일링을 시스템이 처리한다. 모달 루프에서 타이머가 계속 도는 문제는 트레이 메뉴를 고치면서 이미 풀었다. `.rc` 하나로 대화상자 + 매니페스트(comctl32 v6 · DPI) + `VERSIONINFO` + exe 아이콘을 함께 얻는데, 지금 넷 다 없다 |
| `DLGTEMPLATE`을 Rust로 조립 | 빌드 도구는 줄지만 비주얼 스타일에는 매니페스트와 `build.rs`가 어차피 필요하다. 정렬에 민감한 200줄을 리뷰·유지할 이유가 약하다 |
| `native-windows-gui` | 최신 릴리스 1.0.13(2022-09), 저장소 마지막 push 2024-05, 열린 이슈 72개. `winapi` 바인딩을 한 벌 더 들이고, 자체 스케일링이라 모니터 3대 혼합 DPI에서 가장 불리하다 |
| `egui`/`eframe` | 창 하나 때문에 그래픽·폰트 스택이 두 벌이 되고 상주 자원과 exe 크기가 커진다. 490 KB / 18.7 MB / 0.41% 로 기록한 비용 프로필과 충돌한다 |

### 12.3 구조

| 파일 | 상태 | 역할 |
| --- | --- | --- |
| `build.rs` | 신규 | `embed_resource::compile("resources/app.rc")`. `$OUT_DIR/version.rc.gen`에 `CARGO_PKG_VERSION`을 써서 `.rc`가 include |
| `resources/app.rc` | 신규 | `VERSIONINFO` + 매니페스트 참조. `DIALOGEX`는 G3에서 추가 |
| `resources/app.manifest` | 신규 | comctl32 v6 의존성, `PerMonitorV2` DPI, `asInvoker` |
| `src/settings/mod.rs` | 신규 | 진입점, 요청 플래그, 대화상자 수명 |
| `src/settings/model.rs` | 신규 | **순수**: `SettingsDraft`, `Effect`, `validate`, `diff`, 주기 프리셋/표기 |
| `src/settings/dialog.rs` | 신규 | `DialogBoxParamW`, `WM_INITDIALOG`/`WM_COMMAND`/`WM_CTLCOLOR*`, 컨트롤 id 상수 |
| `src/settings/preview.rs` | 신규 | 아이콘 미리보기 스트립(별도 `GlyphCache`, 배경 합성) |
| `src/settings/dark.rs` | 신규 | uxtheme 비공식 API 격리, 브러시, 폴백 |
| `src/ipc.rs` | 신규 | 숨은 최상위 창 + `WM_APP_SETTINGS`. 두 번째 실행을 실행 중 인스턴스로 넘긴다 |
| `src/app.rs` | 변경 | `apply_effects`, `sync_menu_checks`, 대화상자 요청 플래그 |
| `src/main.rs` | 변경 | `--settings`, 요청 플래그 소비, ipc 창 생성·해제 |
| `src/timer.rs` | 변경 | `set_interval` (같은 id로 `SetTimer` 재호출) |
| `src/tray/mod.rs` | 변경 | 메뉴에 `Settings…` 추가, 체크 상태를 한 곳에서 동기화 |
| `src/config.rs` | 변경 | 저장 원자화(임시 파일 + 교체) 권장, 주기 상수 공유 |
| `Cargo.toml` | 변경 | `[build-dependencies] embed-resource`, `Win32_UI_Controls` 기능 추가 |
| `AGENTS.md`, `README.md`, `docs/SPIKE.md` | 변경 | 구조표·메뉴표·실측 기록 갱신 |

**변경 흐름** — 대화상자는 `App`을 빌린 채 열지 않는다.

```
트레이 메뉴 Settings…  ─┐
--settings              ─┼─→ 요청 플래그 → 메시지 루프 → settings::open()
두 번째 실행 (PostMessage) ─┘   (thread local)   (App 대여 없음)
                                                         │
                                          DialogBoxParamW (중첩 루프,
                                          그 안에서 WM_TIMER가 계속 돈다)
                                                         │ WM_COMMAND 1건
                              SettingsDraft → validate → diff → Vec<Effect>
                                                         │ 짧은 runtime::with
                              App::apply_effects → config · 타이머 · 트레이 · 샘플러 · 레지스트리
                                                         │
                                        save_config() 1회 + sync_menu_checks()
```

```rust
pub enum Effect {
    Interval(u64),                 // ms, 범위로 클램프
    Visible(Metric, bool),         // 마지막 지표 규칙은 model에서 판정
    ColorMode(ColorMode),
    CountVirtual(bool),            // = !exclude_virtual_adapters
    Autostart(bool),
}

pub fn diff(current: &Config, autostart: bool, next: &SettingsDraft) -> Vec<Effect>;
```

효과를 한 곳(`App::apply_effects`)에서만 반영하면 창과 트레이 메뉴가 같은 경로를 쓴다.
저장도 한 번만 하므로 지금 메뉴 경로(토글마다 저장)보다 쓰기가 적다.

### 12.4 화면

```
┌─ tray-monitor settings ─────────────────────────────────┐
│  Readouts                                               │
│   [x] CPU   [x] Memory   [x] Network down   [x] Network up │
│                                                         │
│   preview:  ┌────┐┌────┐┌────┐┌────┐   (실제 트레이 크기와 4배 확대) │
│             │CPU ││MEM ││ DN ││ UP │                    │
│             │34% ││41% ││1.7K││341K│                    │
│             └────┘└────┘└────┘└────┘                    │
│                                                         │
│  Update every        [ 1 second        ▼]               │
│  Colours             (•) Per readout   ( ) Taskbar theme │
│  Network             [x] Count virtual adapters          │
│  Startup             [x] Run at sign-in                  │
│                                                         │
│  마지막 지표는 끌 수 없습니다 (상태줄)                    │
│                                                         │
│  [Open config file] [Open log] [Reset to defaults]       │
│                                    v0.3.0      [ Close ] │
└─────────────────────────────────────────────────────────┘
```

- 제목 `tray-monitor settings`, 크기 220×150 DLU 수준, 리사이즈 없음(`WS_CAPTION | WS_SYSMENU`).
- `Escape`와 `Close`가 닫기다. 별도 저장 버튼이 없다(즉시 적용).
- 컨트롤: 체크박스, 콤보(주기), 라디오 2개(색 모드), 상태줄 static, 푸시버튼 3개.
- 열 때 커서가 있는 모니터를 기준으로 배치한다(`MonitorFromPoint`). 기본 동작은 주 모니터에 뜬다.
- 미리보기는 실제 트레이 px로 렌더한 뒤 최근접 업스케일로 확대한다. 업스케일은 순수 함수다.

### 12.5 마일스톤 (예상 5.5~6일)

| 단계 | 내용 | 완료 조건 |
| --- | --- | --- |
| **G0** 리소스 파이프라인 (0.5일, 완료) | `build.rs` + `embed-resource`, `app.rc`(`VERSIONINFO` + 매니페스트), `app.manifest`(comctl32 v6 · PerMonitorV2), 버전 생성 파일 | **완료**(§12.11). `windows.h` include와 `rc.exe` 탐색은 `embed-resource`에 맡긴다. 아이콘 리소스는 G8로 미룸 |
| **G1** 설정 모델 (0.5일) | `model.rs`: `SettingsDraft`, `Effect`, `validate`, `diff`, 주기 프리셋과 표기 변환 | 단위 테스트: 범위 클램프, 마지막 지표 규칙, 최소 효과 diff, 주기 표기 왕복 |
| **G2** 적용 경로 단일화 (0.5일) | `App::apply_effects`, 트레이 메뉴 핸들러를 `Effect` 경유로 변경, `timer::set_interval`, `sync_menu_checks`, 저장 원자화 | 메뉴 토글이 이전과 동일하게 동작. 주기 변경이 재시작 없이 반영. 설정 파일 저장이 1회로 줄어든다 |
| **G3** 대화상자 (1.5일) | `dialog.rs`, `app.rc` 레이아웃, `WM_INITDIALOG` 채우기, `WM_COMMAND` → `Effect`, 대여 밖 진입 | 창에서 바꾼 값이 즉시 반영되고 메뉴 체크가 따라온다. **창이 열려 있는 동안 아이콘이 계속 갱신된다** |
| **G4** 아이콘 미리보기 (0.5일) | `preview.rs`: 실제 px 렌더 + 4배 확대 + 배경 합성, 별도 캐시 | 테마·색 모드·표시 조합이 미리보기에 즉시 반영. 트레이 캐시 통계가 오염되지 않는다 |
| **G5** 바로가기·복원 (0.25일) | Open config file / Open log / Reset to defaults / 버전 표시 | 복원 후 컨트롤·아이콘·파일이 모두 기본값과 일치 |
| **G6** 다크 모드 (1일) | `dark.rs`: `SetPreferredAppMode`·`AllowDarkModeForWindow`·`SetWindowTheme`, `WM_CTLCOLOR*`, `WM_THEMECHANGED`, 트레이 메뉴 `FlushMenuThemes`, `--theme` 진단 | 26H2(26300)에서 창이 어두워지고 라이트에서 회귀 없음. API를 못 찾으면 라이트로 폴백하고 로그 1회 |
| **G7** 진입 경로 (0.5일) | 메뉴 `Settings…`, `--settings`, `ipc.rs`로 재실행 시 창 열기 | exe 재실행 → 실행 중 인스턴스에 설정 창이 뜨고 두 번째 프로세스는 조용히 종료 |
| **G8** 문서·릴리스 (0.5일) | README 메뉴표·설정 절, AGENTS.md 구조표·검증 루프, SPIKE.md에 DPI·다크 모드 실측, PLAN.md §12 결과 기록, 0.3.0 범프 | 태그 `v0.3.0` 릴리스가 자동 생성 |

### 12.6 구현 제약 (어기면 바로 버그)

1. **대화상자를 `runtime::with` 대여 안에서 열지 않는다.** `drain_events`가 `App`을 빌린 채
   모달 루프에 들어가면 그동안 타이머 틱이 전부 `try_borrow_mut` 실패로 버려져 **아이콘이 멈춘다.**
   메뉴 핸들러는 요청 플래그만 세우고, 메시지 루프가 대여 밖에서 연다.
2. 대화상자 프로시저 안의 변경은 **짧은 `runtime::with` 한 번**으로 적용한다. 대화상자
   프로시저가 `App`을 오래 들고 있으면 1번과 같은 정지가 다시 생긴다.
3. **자체 대화상자 위에 `MessageBox`를 띄우지 않는다.** 중첩 모달에 테마도 깨진다. 안내는
   상태줄 static 텍스트로 한다.
4. **마지막 지표 규칙은 한 곳에만.** 트레이 메뉴와 설정 창이 같은 함수(`model`)를 호출한다.
   두 곳에 규칙이 생기면 한쪽이 `config.show.count()==0`을 만들 수 있다.
5. **컨트롤 id는 Rust 상수가 단일 출처.** `include_str!("../resources/app.rc")`로 id가 모두
   존재하는지 테스트한다. 어긋나면 클릭이 조용히 무시된다.
6. **트레이 아이콘을 재생성하지 않는다.** 창이 열렸다고 아이콘을 새로 만들지 않고
   `set_visible`/`set_style`만 쓴다(승격 설정과 id 보존).
7. **미리보기는 별도 `GlyphCache`.** 트레이 캐시를 공유하면 하트비트 로그의 히트율이
   의미를 잃는다. 또한 미리보기는 배경에 **합성해서 불투명 비트맵**으로 만든다(알파 처리 회피).
8. **저장은 `Effect` 묶음당 1회.** GUI 조작마다 파일을 쓰지 않는다.
9. **비공식 API는 `settings/dark.rs`에만.** 실패는 라이트 폴백이고 조용해야 한다.
10. `--settings`와 재실행 신호는 **두 번째 인스턴스가 즉시 종료**되어야 한다(단일 인스턴스 뮤텍스
    획득 이전/이후 순서 주의).

### 12.7 검증

| 대상 | 방법 |
| --- | --- |
| `SettingsDraft`, `validate`, `diff`, 주기 표기 | 순수 함수 단위 테스트 (`cargo test`) |
| `Effect` → `Config`/`Tray`/`Sampler` 반영 | `apply_to_config(&mut Config, &Effect)`를 순수 함수로 떼어 테스트, 나머지는 통합 |
| `.rc` 컨트롤 id | `include_str!` 대조 테스트 |
| 미리보기 업스케일 | 최근접 업스케일 순수 함수 테스트(경계 픽셀) |
| 대화상자 스케일 | 100 / 150 / 200% 모니터를 오가며 수동 확인(모니터 3대 혼합 환경) |
| 다크 모드 | `tray-monitor --theme` 진단 + 수동. 설정 앱에서 테마를 바꿔 `WM_THEMECHANGED` 확인 |
| 핸들 누수 | 창 열고 닫기 10회 전후 `GetGuiResources`를 로그로 비교 |
| 비용 | 창을 닫은 뒤 18.7 MB / 216 핸들 기준선으로 복귀하는지 확인(§10 표에 설정 창 항목 추가) |
| 트레이·메뉴 | 기존 수동 체크리스트 + 설정 창과 체크 상태 동기화 |
| CI | `--theme`를 스모크 테스트에 추가(데스크톱 없이 실행 가능) |

수동 체크리스트(릴리스 전):

- [ ] 창이 열려 있는 동안 아이콘이 계속 갱신된다
- [ ] 체크 해제 즉시 아이콘이 사라지고 트레이 메뉴 체크도 풀린다
- [ ] 마지막 지표를 끄려 하면 되살아나고 상태줄에 이유가 보인다
- [ ] 주기 변경이 재시작 없이 반영된다(하트비트 로그 간격)
- [ ] 색 모드 전환이 미리보기와 트레이에 동시에 반영된다
- [ ] 라이트/다크 전환에서 창과 아이콘이 모두 따라간다
- [ ] 모니터 간 이동에서 잘림·겹침이 없다
- [ ] exe 재실행 → 설정 창이 뜨고 프로세스는 하나만 남는다
- [ ] 설정 파일이 깨져 있어도 기존처럼 `.bak` 백업 후 기본값으로 뜬다

### 12.8 리스크

| 리스크 | 대응 |
| --- | --- |
| CI·로컬에 `rc.exe`가 없으면 빌드 실패 | G0에서 먼저 확인. `windows-2025` + MSVC 툴체인은 충족(vswhere·Windows Kits 10 확인됨). 실패 시 README에 툴체인 요구를 명시 |
| 매니페스트 추가가 DPI 동작을 바꾼다 | 매니페스트의 `dpiAwareness`와 `win32::set_dpi_awareness()` 호출 중 하나로 단일화. G0에서 3개 배율 스크린샷 |
| PMv2 대화상자 스케일이 26H2에서 기대와 다르다 | G3의 실제 대화상자에서 100/150/200% 모니터로 실측. G0은 매니페스트의 DPI 선언이 적용되는 것까지만 확인했다 |
| 다크 모드 ordinal이 빌드에 따라 바뀐다 | 못 찾으면 라이트 폴백. `--theme`로 사용자 환경 상태를 로그에 남긴다 |
| 대화상자 도중 RefCell 대여로 아이콘 정지 | 진입을 대여 밖으로(§12.6-1) + 수동 체크리스트 1번 |
| 설정 창을 닫아도 자원이 남는다 | `GetGuiResources` 로그와 기준선 비교. 미리보기 비트맵은 `DeleteObject` |
| 새 GUI 문자열이 영문/한글 혼재 | 창 문자열은 기존 트레이 메뉴와 같이 영문, 문서는 한국어 유지 |

### 12.9 범위 밖

- 지표별 색 직접 선택(색 대화상자), 폰트·여백 조정
- 아이콘 순서 변경(셸이 관리한다)
- 모델리스 창(항상 떠 있는 창)
- 설정 파일 외부 편집 감지·실시간 리로드
- 다국어 UI

설치 프로그램(MSIX)은 v0.4.0에서 범위에 들어왔고 [PACKAGING.md](PACKAGING.md)에서 다룬다.

### 12.10 열려 있는 결정

1. **폴링 주기 입력 방식** — 프리셋 콤보(250/500/1000/2000/5000/10000) + `Custom…`일 때 edit·스피너를
   권한다. G3에서 확정
2. **`Open config file`을 트레이 메뉴에 남길지** — 권장: 유지(파워유저 경로) + 창에도 버튼
3. **exe 아이콘 디자인** — G8까지 미룰 수 있고, 그전까지 `ICON` 리소스만 비워 둔다. v0.4.0에서
   임시 마크를 넣었고 디자인은 후속에서 교체한다(`tools/make_assets.py`).
4. **설정 파일 저장 원자화** — `MoveFileExW(MOVEFILE_REPLACE_EXISTING)`를 쓰려면
   `Win32_Storage_FileSystem` 기능이 하나 더 필요하다. G2에서 비용 보고 결정

### 12.11 G0 진행 결과

`build.rs`가 `resources/app.rc`를 컴파일해 exe에 `VERSIONINFO`와 매니페스트를 링크한다.
실측 근거와 함정은 [SPIKE.md](SPIKE.md) §10에 있다.

| 항목 | 값 |
| --- | --- |
| exe | 505,344 → **508,928 bytes** (+3,584) |
| `Cargo.lock` | 131 → **138** 패키지 (`cc`, `embed-resource`, `find-msvc-tools`, `shlex`, `vswhom`, `vswhom-sys`, `winreg` — 전부 빌드 전용) |
| 런타임 의존성 | **변화 없음** |
| 로컬 크레이트 재빌드 | 12초 (리소스 컴파일 포함) |
| 품질 검사 | `cargo fmt --check` · `clippy -D warnings` · `test` 97개 · `--dump` BMP 36개 통과 |
| 버전 정보 | `FileVersion 0.2.0`, `FileDescription`, `CompanyName`, `LegalCopyright`, `OriginalFilename`, `ProductName` |
| 매니페스트 | `mt.exe -inputresource:...;#1`로 추출해 comctl32 v6 · `PerMonitorV2` · `asInvoker` 확인 |
| comctl32 v6 | WinSxS `...common-controls_6595b64144ccf1df_6.0.26100.9711...\comctl32.dll` 로드 확인 |
| DPI | 앱 로그가 `DPI: 매니페스트의 PerMonitorV2 선언을 사용한다`를 출력 |

계획과 달라진 점:

1. **`rc.exe`가 `windows.h`를 못 찾는다.** VS 개발자 프롬프트 밖에는 기본 include 경로가 없어
   직접 호출하면 `RC1015`로 죽는다. `embed-resource`가 Windows Kits의 include 디렉터리로
   `%INCLUDE%`를 채워 주므로 `build.rs`에는 경로 처리를 넣지 않았다(SPIKE.md §10-1).
2. **exe 아이콘은 G8로 미뤘다.** 아이콘 디자인이 아직 정해지지 않았고(§12.10-3) 없어도 동작한다.
   그래서 G0 완료 조건에서 아이콘 문구를 뺐다.
3. **comctl32 v6의 파일 버전 문자열은 `5.82`다.** 경로로 판별해야 한다(SPIKE.md §10-2).
4. **대화상자 배율 실측은 G3로 옮겼다.** G0에는 대화상자가 없다. 지금 확인한 것은 매니페스트의
   DPI 선언이 실제로 적용된다는 것까지다(SPIKE.md §10-5).

### 12.12 G1 ~ G7 진행 결과

설정 창을 띄워 컨트롤을 조작하며 확인했다. 실측은 [SPIKE.md](SPIKE.md) §11에 있다.

| 단계 | 결과 |
| --- | --- |
| G1 설정 모델 | `src/settings/model.rs`. draft · `Effect` · `validate` · `diff` · 마지막 지표 규칙이 전부 순수 함수다 |
| G2 적용 경로 단일화 | `App::apply_effects` 하나로 모았다. 트레이 메뉴도 `Effect`를 만들어 같은 길을 탄다. `timer::set_interval`, 메뉴 체크 동기화(`MenuHandles::sync`), 설정 저장을 임시 파일 + 교체로 바꿨다 |
| G3 대화상자 | `resources/app.rc`의 `IDD_SETTINGS` + `src/settings/dialog.rs`. 모든 컨트롤이 즉시 적용되고, 창이 열려 있는 동안에도 트레이가 계속 갱신된다 |
| G4 아이콘 미리보기 | `src/settings/preview.rs`. 트레이와 같은 렌더러로 그려 배경 위에 합성한다(정적 컨트롤은 알파를 합성하지 않는다) |
| G5 바로가기·복원 | Open config file / Open log / Reset to defaults / 버전 표시 |
| G7 진입 경로 | 트레이 메뉴 `Settings…`, `--settings`, exe 재실행 시 실행 중 인스턴스에 창 열기(`src/ipc.rs`) |

계획과 달라진 점:

1. **주기 입력에 스피너(up-down)를 넣지 않았다.** 콤보 + 입력 칸만 둔다. 스피너는
   `Win32_UI_Controls`와 buddy 배선을 더 요구하는데, 콤보로 고르고 숫자로 다듬는 흐름은
   입력 칸만으로 충분하다.
2. **`.rc` 컨트롤 id 대조 테스트를 넣었다.** `include_str!("../../resources/app.rc")`로
   상수와 리소스가 어긋나지 않는지 검사한다(§12.6-5).
3. **검증에서 결함 2개를 찾아 함께 고쳤다.** 자동 실행 해제가 `Access is denied`로 실패하던
   문제와, 그 실패를 알리는 `MessageBox`가 `App` 대여를 붙잡아 트레이를 멈추던 문제다
   (SPIKE.md §11-4). 둘 다 v0.2.0에 있던 것이라 트레이 메뉴 경로도 같이 고쳐졌다.
4. **`Tray::is_visible`과 `Slot::visible`을 지웠다.** 표시 여부는 `config.show` 하나로
   충분해져 중복 상태가 사라졌다.

### 12.13 G6 ~ G8 진행 결과

| 단계 | 결과 |
| --- | --- |
| G6 다크 모드 | `win32::dark`(uxtheme 비공식 ordinal 133/135/136) + `WM_CTLCOLOR*` + `WM_THEMECHANGED`. 트레이 우클릭 메뉴도 함께 어두워진다. 실측은 [SPIKE.md](SPIKE.md) §11-5 |
| G8 문서·릴리스 | README에 설정 창 절과 `--theme`, AGENTS.md에 구조와 함정 2개 추가, 0.3.0 범프 |

계획과 달라진 점:

1. **다크 모드는 `src/settings/dark.rs`가 아니라 `src/win32.rs`의 `dark` 모듈에 있다.** 전부
   `unsafe` Win32 호출이라 "`unsafe`는 `gdi.rs`와 `win32.rs`에만" 규칙을 지키려면 그쪽이 맞다.
2. **`--theme` 진단을 넣고 CI 스모크 테스트에 추가했다.** ordinal이 빌드마다 있는지 확인할 수
   있고, 다른 Windows 빌드에서 도는 러너에서도 검증된다.
3. **다크 모드에 스택 오버플로 함정이 있었다.** `SetPreferredAppMode`와 `SetWindowTheme`이
   테마 메시지를 다시 보내는데, 그 메시지에서 다시 테마를 입히면 재진입이 끝없이 쌓여
   프로세스가 죽는다(build 26300에서 실측). 재진입 가드를 두어 해결했다(SPIKE.md §11-5).

---

## 13. 패키징 (v0.4.0)

- **Installer:** Inno Setup 6, 사용자 단위 설치(`%LOCALAPPDATA%\Programs\tray-monitor`),
  `/ALLUSERS`로 per-machine 전환. 업그레이드 전에 IPC 창에 `WM_CLOSE`를 보내 실행 중인 앱을
  정상 종료시킨다.
- **MSIX:** full-trust(`runFullTrust`), `uap5:startupTask`, 자산은 `resources/msix/`.
  서명은 아직 없다. Store identity는 예약된 `tinyrack.TinyTrayManager` /
  `CN=42FA8D93-3683-4A00-A40C-5ED8A02E8B3A` 기본값을 환경 변수로 덮어쓴다.
- **CI:** `package` 잡이 installer 무인 설치·제거와 `makeappx` 왕복을 검증하고, Quality Gate가
  이를 요구한다. 릴리스는 zip + setup.exe + msix + `SHA256SUMS`를 올린다.
- **런타임:** 패키지에서는 설정/로그를 `LocalState`에 쓰고(`src/platform.rs`) 자동 시작을
  `StartupTask`로 처리한다. 승격 경로는 `\WindowsApps\` 뒤만 비교하고, 지표별 고정 GUID로
  등록해 업데이트 후에도 승격을 유지한다.

실측 근거, 위험, 검증 절차는 [PACKAGING.md](PACKAGING.md)에 있다.
