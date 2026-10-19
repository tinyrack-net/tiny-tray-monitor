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

- 태그 `v*.*.*` → 빌드 → 포터블 zip + `SHA256SUMS` → `gh release create --generate-notes`
- winget / scoop 매니페스트는 후속 (조직에 `setup_winget` 롤이 있다)
- **완료 조건**: `v0.1.0` 태그로 릴리스가 자동 생성되고 산출물이 실행된다

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
4. **exe 경로를 바꾸지 않는다.** 트레이 승격 설정이 경로에 묶여 있어, 버전별 폴더로 업데이트하면
   사용자의 승격이 매번 초기화된다. 자동 업데이트는 제자리 교체로 설계한다.
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
