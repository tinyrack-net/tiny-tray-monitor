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
| 지표 | CPU, 메모리, 네트워크, GPU, 전용 VRAM | 관리자 권한·커널 드라이버 불필요 |
| 표시 | 지표별 아이콘 1개, **라벨 + 값 2줄** | 타입 식별을 위해 가독성을 일부 양보 |
| 렌더 | 1비트 픽셀 폰트(캡션 3x5 / 값 4x10) + nearest-neighbor 배율 | GDI/AA 없이 결정로적, 배율별 글리프 불필요 |
| 값 캐시 | 표시 문자열 단위 캐시 | 값이 유한 집합이라 히트율이 높다 (코어 0.32→0.15%) |
| 플랫폼 | Windows 전용 | 지표 수집은 Win32, 렌더는 순수 함수(GDI는 미리보기 DIB만) |

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

| 지표 | 캡션 | 값 예 | 규칙 |
| --- | --- | --- | --- |
| CPU | `CPU` | `80` | 0~100. `%`는 캡션 이름이 대신하므로 그리지 않는다 |
| MEM | `MEM` | `77` | 동일 |
| NET ↓ | `↓MB` | `24` | 다운로드 속도. 배율(B/K/M/G)만 캡션에 올린다 |
| NET ↑ | `↑KB` | `1.7` | 업로드 속도 |
| GPU | `GPU` | `14` | 가장 바쁜 엔진의 사용률(0~100) |
| VRAM | `VRM` | `90` | 전용 VRAM 사용률(0~100) |

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
│   │   ├── glyphs.rs         # 1비트 글리프 표 (tools/pixelfont.py 생성)
│   │   ├── pixel.rs          # 글리프 -> 비트맵 (순수, GDI 없음)
│   │   ├── layout.rs         # master 레이아웃 정책과 지표 (순수)
│   │   ├── render.rs         # Bitmap과 nearest-neighbor 배율
│   │   └── cache.rs          # 표시 문자열 캐시
│   ├── tray/
│   │   └── mod.rs            # 슬롯 관리, 우클릭 메뉴, 고정 GUID 등록
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

1. **빈 문자열을 렌더 진입점에서 거른다.** GDI `DrawTextW` 시절에는 프로세스가 즉시 죽었다. 지금 `pixel::render_line`은 `None`을 돌려준다.
2. **DIB의 알파 바이트를 읽지 않는다.** GDI는 B, G, R만 채운다. 남은 `gdi::bitmap_from_rgba`(미리보기)에만 해당한다.
3. **`SetTimer(hwnd=NULL)`의 반환값으로 타이머를 식별한다.** 요청한 ID는 무시된다.
4. **exe 경로를 바꾸지 않는다.** 트레이 승격 설정은 기본적으로 경로에 묶여 있어, 버전별
   폴더로 업데이트하면 초기화된다. installer 자동 업데이트는 제자리 교체로 설계한다.
   MSIX처럼 경로가 바뀔 수밖에 없는 채널은 지표별 고정 GUID로 승격을 유지한다.
5. **등록 시점의 툴팁은 고정 문자열로 둔다.** 승격 설정은 툴팁 변경에 안전한 것으로 확인했지만,
   등록 시점 문자열은 사용자에게 보이는 최초 식별자이므로 살아있는 값을 넣지 않는다.
6. **표시 문자열 캐시를 유지한다.** 픽셀 렌더러는 GDI보다 싸지만, 매 갱신마다 비트맵을 새로 할당하지 않도록 캐시를 둔다.

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
| 아이콘 렌더 | 1비트 픽셀 폰트라 **픽셀 해시 비교가 가능하다.** 글리프 표 정의, 문자열 폭, 1비트(회색 없음), 배율별 불변식을 검사한다 |
| 지표 수집 | 실제 API를 호출하는 스모크 테스트(`--probe`)로 대체. CI에서는 값의 범위만 검사 |
| 트레이 | 수동 검증 체크리스트 (docs/design/icon.md에 둔다) |
| 아이콘 가독성 | `tools/icon_sheet.py`로 레이아웃 시트를 만들어 눈으로 판정. 새 레이아웃을 추가할 때마다 갱신 |

`make validate` = `fmt --check` + `clippy -D warnings` + `test` + `build --release`.

---

## 8. 범위 밖 (v1)

- CPU 온도 / 팬 / 전압: 관리자 권한과 커널 드라이버가 필요하다. 별도 관리자 헬퍼 + IPC로 후속 검토
- GPU: v1에서 제외했다가 **v0.7.0에서 추가했다**(§15). 관리자 권한이 필요 없다
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
2. **모달 대화상자가 뜨면 프로세스가 조용히 죽었다.** 당시 첫 실행 안내가 모달 루프를
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
│  [Reset to defaults]                                     │
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
- ~~다국어 UI~~ → §15에서 다룬다(v0.7.0)

설치 프로그램(MSIX)은 v0.4.0에서 범위에 들어왔고 [PACKAGING.md](PACKAGING.md)에서 다룬다.

### 12.10 열려 있는 결정

1. **폴링 주기 입력 방식** — 프리셋 콤보(250/500/1000/2000/5000/10000) + `Custom…`일 때 edit·스피너를
   권한다. G3에서 확정
2. **`Open config file`을 트레이 메뉴에 남길지** — v0.5.0까지 메뉴와 창 양쪽에 두었다가
   제거했다. 설정 창이 모든 항목을 다루므로 파일을 직접 열 경로는 두지 않는다. 같은 이유로
   설정 창의 `Open log`와 트레이 메뉴의 `Run at sign-in`·`Count virtual adapters`도 뺐다.
   트레이 메뉴에는 지표 표시, `Settings…`, `Exit`만 남는다. 로그는 영문으로만 쓴다
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

---

## 14. 트레이 아이콘 픽셀 폰트 (v0.4.0)

GDI 8배 슈퍼샘플 + 다운샘플 렌더러는 16px에서 라벨(5px)을 AA 회색으로 뭉갰다. 1비트
픽셀 폰트로 교체했다. 계획과 실측은 [PIXEL-FONT.md](PIXEL-FONT.md).

### 구조

- **글리프 표**: `tools/pixelfont.py`가 ASCII art를 `src/icon/glyphs.rs`로 생성한다.
  캡션 3x5, 값 4x10 두 표로 만든다. `--check`가 package 잡에서 돈다.
- **렌더러**: `src/icon/pixel.rs`가 글리프를 1비트로 블릿한다. GDI를 쓰지 않아 순수
  함수이고, 픽셀 해시 테스트가 가능하다.
- **배율**: 16px master 하나를 `render::scale_nearest`로 늘린다. 정수배(16/32/48)는
  균일하고, 비정수(20/24/28)는 1비트를 유지한 채 획 두께만 ±1px 달라진다.
- **캡션 단위**: 단위는 값 뒤가 아니라 캡션 줄에 둔다(`CPU`/`MEM`/`↓MB`/`↑KB`). 값은
  항상 3글자 이하라 4x10 숫자를 1px 간격으로 쓴다. [PIXEL-FONT.md](PIXEL-FONT.md) §9.
- **라벨·단위 토글 제거**: v0.5.0의 `show_labels`/`show_units`는 캡션 개편으로 의미가
  없어져 메뉴·설정 창·`config.toml`에서 뺐다. 예전 설정 파일의 두 키는 읽을 때 버린다.
- **미리보기**: `settings/preview.rs`가 같은 렌더러를 쓰므로 자동으로 새 폰트를 탄다.

### 실측

| 항목 | 결과 |
| --- | --- |
| 16px master | 캡션 3x5 + 간격 1 + 값 4x10 = 16px |
| 폭 예산 | 값 `100`/`999` = 14px, `1.2` = 12px, 캡션 `MEM`/`↓MB` = 15px (16px 안) |
| 계조 | 모든 배율에서 `[190(라벨), 255(값)]` — 회색 없음 |
| GDI 네이티브 비AA | 폰트/크기 모두 2계조지만 `80%`가 18~20px로 16px를 넘침 |

### 되돌아본 결정

- **네트워크 캡션은 화살표 + 단위다.** 처음에는 `DN`/`UP`이 더 명확하다고 봤지만,
  단위를 캡션으로 올리면서 3글자 안에 방향을 넣으려고 `↓`/`↑`로 바꿨다
  ([PIXEL-FONT.md](PIXEL-FONT.md) §9).
- **DPI별 master는 채택하지 않는다.** 유지보수가 급증하고, 정수배로 안 떨어지는
  20/28/40은 어차피 비정수가 남는다. master는 하나다.

---

## 15. 다국어 UI와 설정 레이아웃 개편 (v0.7.0)

설정 창은 v0.3.0에서 만든 뒤 값을 늘리기만 했다(`color_mode`, `exclude_virtual_adapters`,
지표 4종). 컨트롤이 설명 없이 나열돼 각 항목이 무슨 역할인지 알기 어렵고, 문자열이 전부
영어로 박혀 있다. 설정 창 레이아웃을 그룹 + 설명 구조로 다시 짜고, **영어/한국어/일본어**
UI를 넣는다. 범위는 사용자에게 보이는 문자열 전체다(설정 창, 트레이 메뉴·툴팁, 첫 실행
안내, 시작 실패·중복 실행 알림). 로그는 규칙대로 영문을 유지한다.

### 15.1 확정된 결정

| 항목 | 결정 | 근거 |
| --- | --- | --- |
| 언어 목록 | `Auto` / English / 한국어 / 日本語 | `Auto`는 `GetUserDefaultUILanguage`로 해석한다. 신규 설치가 OS 언어를 따르고, 사용자가 고정할 수도 있다 |
| 저장 위치 | `Config.language`(serde default `Auto`) | 기존 `config.toml`과 호환된다. 해석된 값이 아니라 **선택을** 저장한다 |
| 문자열 구조 | `struct Text`의 `&'static str` + 소수의 `fn` 포인터, `const EN / KO / JA` | 필드가 하나라도 빠지면 컴파일이 실패한다. 런타임 조회·해시맵이 없어 비용이 0에 가깝다 |
| 리소스 | `.rc`는 텍스트가 빈 placeholder 템플릿 **하나만** 유지 | 레이아웃 단일 소스. 다국어 `.rc` LANGUAGE 블록은 런타임 전환이 안 되고 레이아웃이 3벌로 갈라진다 |
| 전환 시점 | 창을 닫지 않고 **즉시 반영** | 트레이 메뉴 `set_text`, 콤보 재구성, 툴팁은 다음 틱, 미리보기 재렌더 |
| 레이아웃 | 단일 페이지 + 그룹 섹션 + 그룹 설명 + 컨트롤 힌트 | 항목이 7개 수준이라 탭·스크롤이 필요 없다. 설명이 항상 보인다 |
| 문자열 소유 | `src/i18n.rs` 하나에 모은다 | 로그용 영문 검사(`logging::tests::source_strings_are_english`)의 예외를 한 파일로 격리한다 |

### 15.2 구조

| 파일 | 상태 | 역할 |
| --- | --- | --- |
| `src/i18n.rs` | 신규 | **순수**: `Language`, `Text`, `EN`/`KO`/`JA`, `resolve(system_lang)`, `auto()`, `interval_label`, `metric_tooltip` |
| `src/config.rs` | 변경 | `pub language: Language`(serde default `Auto`), `validate`에서 미지원 값 정리 |
| `src/settings/dialog.rs` | 변경 | 컨트롤 텍스트 주입(`apply_text`), 언어 콤보, 전환 시 재적용, 상태줄 문구를 `Text`로 |
| `src/settings/model.rs` | 변경 | `Effect::Language(Language)`, `SettingsDraft.language`, 상태 문구를 `Text`로 |
| `resources/app.rc` | 변경 | 그룹·설명 LTEXT 추가, 정적 라벨에 id 부여, 텍스트는 빈 값, 창 크기 확대 |
| `src/tray/mod.rs` | 변경 | 메뉴·`about` 텍스트를 `Text`에서, `MenuHandles::set_text`로 갱신 |
| `src/main.rs` | 변경 | 중복 실행·시작 실패 알림을 `Text`로, 시작 시 언어 해석 |
| `src/icon/layout.rs` | 변경 | `tooltip_name`을 `i18n`으로 이관(아이콘 캡션 `CPU`/`MEM`/`↓MB`는 그대로) |
| `src/logging.rs` | 변경 | 검사 목록에서 `i18n.rs`를 빼고, 그 자리를 i18n 전용 테스트로 채운다 |

```rust
pub enum Language { Auto, En, Ko, Ja }        // Config 저장값
pub enum Lang { En, Ko, Ja }                   // 해석된 값, Text 조회 키

pub struct Text {
    pub window_title: &'static str,
    pub group_readouts: &'static str,
    pub readouts_hint: &'static str,
    // …컨트롤·버튼·상태 문구
}

pub const EN: Text = Text { … };
pub const KO: Text = Text { … };
pub const JA: Text = Text { … };

pub fn resolve(system_lang: u16) -> Lang;   // 순수, 단위 테스트
pub fn text(lang: Lang) -> &'static Text;
```

### 15.3 문자열 인벤토리

이관 대상은 다음과 같다. 문자열 키는 `Text`의 필드이고, 각 언어 표에 같은 필드가 모두
있어야 컴파일된다.

| 화면 | 현재 위치 | 키(예) |
| --- | --- | --- |
| 설정 창 제목·그룹·설명 | `resources/app.rc` | `window_title`, `group_readouts`, `readouts_hint` … |
| 컨트롤 라벨·힌트 | `resources/app.rc` | `metric_label`, `update_hint`, `count_virtual_hint` … |
| 주기 콤보 | `settings/model.rs` | `interval_label(ms)`, `custom_interval` |
| 상태줄·오류 | `settings/dialog.rs`, `settings/model.rs` | `last_readout_off`, `interval_range`, `reset_done`, `save_failed` |
| 트레이 메뉴 | `tray/mod.rs` | `menu_settings`, `menu_exit`, `menu_about` |
| 툴팁 | `icon/layout.rs` | `metric_tooltip(metric)` |
| 시작 실패·중복 실행 | `main.rs` | `already_running`, `start_failed` |

**번역하지 않는 것:** 지표 캡션(`CPU`/`MEM`/`↓MB`/`↑KB`), 제품명 `TinyTrayManager`,
버전 문자열, 로그, `config.toml` 키와 값, 진단 출력(`--probe`/`--dump`).
`인터벌`처럼 아이콘이 아니라 텍스트 UI인 경우(주기 콤보)는 번역한다. 언어 이름은 자기
언어로 표기한다(`English` / `한국어` / `日本語`).

### 15.4 화면

창 크기를 248×188 → **300×252 DLU**로 넓힌다(영어가 가장 길다). 그룹마다 설명 한 줄,
애매한 항목에는 오른쪽 힌트를 붙인다.

```
┌ TinyTrayManager settings ──────────────────────────────┐
│ ┌ Readouts ──────────────────────────────────────────┐ │
│ │ Which values appear in the taskbar.                 │ │
│ │  [x] CPU            [x] Memory                      │ │
│ │  [x] Network down   [x] Network up                  │ │
│ │  ┌────────────── icon preview ──────────────────┐   │ │
│ └─────────────────────────────────────────────────────┘ │
│ ┌ Update ────────────────────────────────────────────┐ │
│ │ How often the values are sampled.                   │ │
│ │  Every [ 1 second ▼ ]  [ 1000 ] ms   (250–10000)    │ │
│ └─────────────────────────────────────────────────────┘ │
│ ┌ Appearance ────────────────────────────────────────┐ │
│ │ Colour of the icon text.                            │ │
│ │  ( ) Per readout   ( ) Taskbar theme                │ │
│ └─────────────────────────────────────────────────────┘ │
│ ┌ Startup & language ────────────────────────────────┐ │
│ │  [x] Count virtual adapters   (VPN / Hyper-V)       │ │
│ │  [x] Run at sign-in                                 │ │
│ │  Language [ English ▼ ]                             │ │
│ └─────────────────────────────────────────────────────┘ │
│ <status line>                                v0.7.0     │
│ [ Reset to defaults ]                          [ Close ] │
└─────────────────────────────────────────────────────────┘
```

- 정적 라벨도 `SetDlgItemTextW`로 채우려면 id가 필요하다. `.rc`의 `-1`을 없애고
  `IDC_*` 상수를 부여한다(`control_ids_match_the_resource_file` 테스트를 확장).
- 그룹박스는 `BS_GROUPBOX`, 설명은 읽기 전용 `LTEXT`. 컨트롤 힌트는 회색 계열로 두되 다크
  모드에서 대비를 확인한다(`WM_CTLCOLORSTATIC`).
- 폰트는 `Segoe UI` 9pt를 유지하고, CJK는 Windows 폰트 링킹(맑은 고딕 / Yu Gothic)에 맡긴다.
  L0에서 한글·일본어 글리프가 실제로 대체되는지 확인하고, 안 되면 대화상자 폰트를
  `Malgun Gothic`/`Yu Gothic UI`로 바꾸는 폴백을 둔다.

### 15.5 마일스톤 (예상 4일)

| 단계 | 내용 | 완료 조건 |
| --- | --- | --- |
| **L0** 스파이크 (0.5일, 완료) | ① `WM_INITDIALOG`에서 전 컨트롤 텍스트 주입 + 런타임 전환이 다크 모드/PMv2에서 안전한지 ② CJK 폰트 링킹 ③ `muda` `MenuItem::set_text` 동작 | **완료.** 실측은 [SPIKE.md](SPIKE.md) §13. 셋 다 가능: 런타임 주입/실시간 전환이 동작하고, Segoe UI 폰트 링킹이 CJK를 대체하며, `set_text`가 있다. 창은 527x572px(100%) |
| **L1** i18n 코어 (1일) | `src/i18n.rs`: `Language`/`Lang`/`Text`/`EN`·`KO`·`JA`, `resolve`, 주기 표기·툴팁·승격 문구 이관, `Config.language`, 영어 검사 목록 조정 | 단위 테스트: 세 언어 필드 비어 있지 않음, 영어 표에 한글 없음, 자리표시자 개수 일치, `Auto` 해석 경계(ko/ja/그 외) |
| **L2** 문자열 이관 (0.5일) | 설정 창·모델·트레이 메뉴·툴팁·온보딩·`main.rs` 알림을 `Text`로 교체. `Effect::Language` 추가 | 영문 빌드가 기존과 같은 문구. `make validate` 통과 |
| **L3** 레이아웃 (1.5일) | `.rc` 재작성(그룹·설명·id·300×252), `dialog.rs` `apply_text` + 언어 콤보 + `WM_APP_REFRESH` 경로 | 세 언어로 창을 열어 잘림·겹침이 없다. 컨트롤 id 대조 테스트 확장 |
| **L4** 실시간 반영·검증 (0.5일) | 전환 시 창·미리보기·트레이 메뉴 즉시 갱신, 툴팁은 다음 틱, 언어별 수동 체크리스트 | 세 언어 × 라이트/다크 × 100/150/200% 확인. `make dump` 회귀 없음 |
| **L5** 문서·릴리스 (0.5일) | README `Settings window` 표에 Language 행, AGENTS.md 구조표·함정(i18n 검사 예외), SPIKE.md 기록, PLAN.md §15 결과, 0.7.0 범프 | 태그 `v0.7.0` 릴리스 |

### 15.6 구현 제약 (어기면 바로 버그)

1. **`i18n.rs`만이 한글·일본어 리터럴을 가진다.** `logging::tests::source_strings_are_english`
   목록에서 `i18n.rs`를 빼고, 대신 `i18n.rs`의 **영어 표에는 한글이 없어야 한다**는 테스트를
   추가한다. 다른 파일에 번역 문자열을 흩으면 로그 검사의 의미가 사라진다.
2. **언어를 바꿔도 트레이 아이콘을 재생성하지 않는다.** 메뉴 텍스트는 `set_text`,
   아이콘은 그대로 둔다(승격 설정과 아이콘 id 보존, AGENTS.md 함정 1·17).
3. **언어 전환은 `App` 대여 밖에서.** 콤보 `CBN_SELCHANGE`는 값만 커밋하고, 화면 갱신은
   기존 `WM_APP_REFRESH`처럼 메시지로 미룬다(§12.6-1).
4. **콤보를 다시 채울 때는 비우고 다시 넣는다.** 주기 콤보는 전환 시 선택 위치가 밀리므로
   `CB_RESETCONTENT` 후 프리셋 + `Custom…`을 다시 넣고 현재 값을 다시 고른다.
5. **`Config.language`는 선택값(`Auto` 포함)을 저장한다.** 해석된 `Lang`을 저장하면 OS 언어를
   바꿔도 `Auto`가 고정된다. 마이그레이션은 serde default로 충분하다.
6. **툴팁 로케일은 다음 틱에 반영된다.** 툴팁 문자열이 바뀌면 `dirty` 규칙(`slot.tooltip`
   비교)이 이미 다시 그리므로 별도 경로를 만들지 않는다.
7. **레이아웃은 영어 길이를 기준으로 잡는다.** 한국어·일본어는 더 짧거나 비슷하다. DLU를
   하드코딩하므로 번역이 넘치면 `.rc`를 고친다(런타임 오토사이즈 없음).

### 15.7 검증

| 대상 | 방법 |
| --- | --- |
| `Text` 표 완전성 | 세 언어 표의 모든 필드가 비어 있지 않음, 영어 표에 비ASCII 없음, 포맷 인자 개수 일치 |
| `resolve`/`auto` | 순수 함수 단위 테스트: `LANG_KOREAN`/`LANG_JAPANESE`/그 외 → `Ko`/`Ja`/`En` |
| `Config.language` 왕복 | serde default `Auto`, `Effect::Language` → `Config` 반영 |
| `.rc` 컨트롤 id | `include_str!` 대조 테스트 확장(새 정적 라벨 전부) |
| 언어 전환 | 세 언어로 열어 창·미리보기·트레이 메뉴가 즉시 바뀌고 툴팁이 다음 틱에 바뀌는지 |
| 표시 품질 | 언어별 × 라이트/다크 × 100/150/200% 스크린샷. 잘림·겹침 없음 |
| 회귀 | `make validate`(fmt·clippy·test), `make dump` 아이콘 회귀 없음 |

수동 체크리스트(릴리스 전):

- [ ] English / 한국어 / 日本語 각각에서 창 텍스트가 잘리지 않는다
- [ ] 언어를 바꾸면 트레이 메뉴와 툴팁이 즉시 새 언어로 바뀐다
- [ ] 언어를 바꿔도 트레이 승격 설정과 아이콘 순서가 유지된다
- [ ] `Auto`로 두고 Windows 표시 언어를 바꾸면 다음 실행부터 따라간다
- [ ] 다크 모드에서 그룹·설명 텍스트가 읽힌다
- [ ] 기존 `config.toml`(language 키 없음)이 `Auto`로 열린다

### 15.8 리스크

| 리스크 | 대응 |
| --- | --- |
| `Segoe UI` 폰트 링킹이 CJK를 대체하지 못한다 | L0에서 확인. 실패 시 대화상자 `FONT`를 `Malgun Gothic`/`Yu Gothic UI`로 바꾸거나 텍스트 주입 후 `WM_SETFONT` |
| 번역이 DLU 레이아웃을 넘친다 | 영어 기준으로 잡고(가장 길다), L3에서 세 언어 스크린샷으로 확인. 넘치면 `.rc` 값 조정 |
| 문자열이 여러 파일로 다시 샌다 | `i18n.rs` 단일 소유 + 영어 검사 목록 조정 + 리뷰 체크 |
| 언어 전환이 다크 모드 재진입 가드와 충돌 | 전환도 `apply_theme`와 같은 가드 경로를 타게 한다(§12.6, AGENTS.md 함정 13) |
| `.rc` 텍스트가 비어 초기 프레임이 깜빡인다 | `WM_INITDIALOG`에서 창을 보이기 전에 모두 채운다. 필요하면 `WS_VISIBLE` 없이 뜨게 하고 채운 뒤 표시 |

### 15.9 진행 결과 (L0 ~ L5)

| 단계 | 결과 |
| --- | --- |
| L0 스파이크 | [SPIKE.md](SPIKE.md) §13. 런타임 텍스트 주입·실시간 언어 전환·CJK 폰트 링킹 모두 실측 확인 |
| L1 i18n 코어 | `src/i18n.rs`: `Language`/`Lang`/`Text`/`EN`·`KO`·`JA`, `from_lang_id`, `interval_label`, 툴팁·승격·알림 문구. `Config.language`. 자리표시자 집합 검사 |
| L2 문자열 이관 | 설정 창·`model`·트레이 메뉴·툴팁·온보딩·`main.rs` 알림이 `Text`를 쓴다. `Effect::Language` 추가. `model::Note`·`LastReadout`로 문구를 값에서 분리 |
| L3 레이아웃 | `.rc`를 300x288 DLU, 그룹 4개 + 설명 + 정적 라벨 id로 재작성. `dialog.rs`의 `apply_text`가 언어를 채우고 언어 콤보를 다시 만든다 |
| L4 검증 | 영어/한국어/일본어로 라이트/다크 확인. UIA로 실시간 전환·재시작 반영·`config.toml` 저장 확인 |
| L5 문서·릴리스 | README 언어 행, AGENTS.md 구조·함정(i18n 검사 예외), SPIKE.md §13, 0.7.0 범프 |

계획과 달라진 점:

1. **`hint_startup`은 컨트롤 힌트까지 한 줄에 담았다.** 가상 어댑터·자동 실행·언어 설명을
   한 문장으로 묶어 그룹 설명 한 줄로 처리하고, 항목별 힌트는 가상 어댑터와 언어에만 둔다.
2. **`SettingsDraft::validate`가 문자열이 아니라 `Note`를 돌려준다.** 같은 메모를 로그에는
   영문, 상태줄에는 지금 언어로 보여 줘야 해서다.
3. **`Metric::tooltip_name`을 없애고 `i18n::metric_label`로 옮겼다.** 아이콘 모듈이 문자열
   표를 모르게 유지하고, 메뉴·체크박스·툴팁이 한 함수를 쓴다.
4. **창 높이가 100%에서 572px다.** 1080p 200%에서 작업 영역을 넘을 수 있다([SPIKE.md](SPIKE.md) §13-3).

### 15.10 범위 밖

- 제3언어(중국어 등) 추가, 번역 파일 외부화(JSON/`.lang`), 사용자 번역 기여 절차
- 날짜·숫자·단위의 로케일 포맷(`1,024`→`1.024`) — 아이콘 값은 로케일과 무관하게 고정
- 아이콘 캡션·지표 약어의 번역(`CPU`/`MEM`/`↓MB`)
- RTL 언어, 시스템 언어 변경의 즉시 감지(`WM_SETTINGCHANGE` 후 자동 전환)
## 16. GPU 사용률 / 전용 VRAM (v0.8.0)

계획에서 v1 범위 밖으로 두었던 지표(§8)를 관리자 권한 없이 추가했다. 실측 근거는
[SPIKE.md](SPIKE.md) §14.

### 결정

| 항목 | 결정 | 이유 |
| --- | --- | --- |
| 수집 | `DXGI`로 어댑터·전용 한도, `gdi32!D3DKMTQueryStatistics`로 엔진 시간·VRAM 상주량 | 관리자 권한도 커널 드라이버도 필요 없다 |
| PDH | 쓰지 않는다 | 이 기기에서 `GPU Engine` 카운터가 갱신되지 않았다(인코더 포화 시에도 11%) |
| 사용률 | 노드(엔진)별 델타 중 **최대값** | 아이콘 하나로 GPU 전체를 요약해야 한다. `nvidia-smi`의 엔진 값과 교차 검증했다 |
| VRAM | aperture가 아닌 세그먼트의 `BytesResident` 합 ÷ DXGI 전용 한도 | PDH `GPU Adapter Memory\Dedicated Usage`와 0.2% 안에서 일치한다 |
| 대표 어댑터 | 소프트웨어 렌더러를 빼고 **전용 메모리가 가장 큰** 어댑터 | 화면을 그리는 GPU와 다를 수 있다(README의 한계에 적었다) |
| 지표 등록 | `Metric::ALL` **끝에** 붙이고 고정 GUID 2개를 새로 발급 | 순서가 바뀌면 기존 승격 설정이 어긋난다(§10) |
| 기본값 | 둘 다 `true` | 다른 지표와 같다. 기존 사용자는 트레이 아이콘 2개를 새로 받고, Windows가 오버플로우에 넣는다 |
| 단위 | 100ns | WDK 주석은 micro-second지만 실측은 100ns다 |

### 구현 메모

- 값 표기: 캡션은 `GPU`/`VRM`(3글자), 값은 다른 지표와 같이 최대 3글자다(§1).
- 글리프 `V`/`R`을 추가했다(`tools/pixelfont.py` → `src/icon/glyphs.rs`).
- `D3DKMT_QUERYSTATISTICS`는 `windows` 크레이트에 정의가 없어 크기와 오프셋을 손으로
  적었다. `kmt_statistics_layout_matches_the_wdk`가 x64 배치를 고정한다.
- 수집 실패(어댑터 없음·드라이버 없음)는 `Option`으로 전파하고, 값이 없으면 아이콘을
  갱신하지 않는다. 계산은 `percent_from_delta`·`busiest_node_percent`·`vram_percent`로
  분리해 단위 테스트가 덮는다.

### 검증

| 항목 | 방법 | 결과 |
| --- | --- | --- |
| 사용률 | NVENC 포화 + `nvidia-smi utilization.encoder` 교차 | 99% ↔ 아이콘 99.8% |
| VRAM | PDH `GPU Adapter Memory(*)\Dedicated Usage` 교차 | 6,958 MB ↔ 아이콘 87.5%(6,962 MB) |
| 배치 | WDK `C_ASSERT(sizeof == 0x328)` | `size_of`/`offset_of` 테스트로 고정 |
| 아이콘 | `make dump` + `tools/icon_sheet.py` | 다크/라이트, 네 크기에서 `GPU`/`VRM` 판독 확인 |
| 트레이 | `make run`, 설정 창, `--probe` | 아이콘 6개·메뉴 6항목·로그 heartbeat 확인 |

남은 일: Store 스크린샷 재촬영(`resources/store/screenshots/02-tray.png`는 아직 4개다).

---

## 17. 트레이 고정 정체성 수정 (v0.8.0)

지표를 4개에서 6개로 늘리자 작업 표시줄에 고정해 둔 아이콘이 풀렸다. 원인은 지표 추가가
아니라 **고정 GUID 등록이 매 실행마다 실패해 경로 기반으로 폴백하던 것**이었다. 재현과
실측은 [SPIKE.md](SPIKE.md) §15.

### 원인

- 셸은 `NotifyIconSettings` 키를 **처음 보는 정체성에만** 쓴다. 이미 아는 정체성에는 쓰지 않는다.
- `Tray::build`는 키가 생기는지를 GUID 수락 판정으로 썼다 → 두 번째 실행부터 항상 폴백.
- 폴백은 GUID 시도가 소비한 uID 뒤에서 다시 만들어져 `N+1..2N`을 받는다. 경로 기반 정체성은
  `(실행 경로, uID)`이므로 지표 수가 바뀌면 기존 고정이 다른 아이콘으로 가거나 사라진다.

### 수정

| 항목 | 변경 |
| --- | --- |
| 수락 판정 | `NIM_ADD` + `set_guid_tooltip`(`NIF_TIP \| NIF_GUID`) 성공. 키 생성 여부는 폴백 사유가 아니다 |
| 진단 | 없음. 앱은 `NotifyIconSettings`를 읽지 않는다(고정 여부는 사용자 결정) |
| 영구 문서 | [SPIKE.md](SPIKE.md) §15, [PACKAGING.md](PACKAGING.md) §10, AGENTS.md 규칙 17·24, `tools/traykeys.py` |

### 검증

| 단계 | 결과 |
| --- | --- |
| Explorer 재시작 → 실행 | 폴백 경고 없음, 6개 키에 `IconGuid` 기록 |
| `tools/promote.py`로 4개 고정 | 작업 표시줄에 4개 표시 |
| exe를 다른 폴더로 복사해 실행 | 같은 키가 새 경로로 이동, `IsPromoted=1` 유지 |

`taskkill /F`로 죽이면 그 Explorer 세션에서는 키가 다시 기록되지 않는다. 정상 종료 경로를
쓴다(문서화했다).
