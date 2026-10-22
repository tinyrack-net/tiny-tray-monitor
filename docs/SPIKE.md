# 트레이 시스템 모니터 스파이크 결과

iStat Menus 스타일의 Windows 트레이 시스템 모니터를 **어떤 방법으로 구현할 수 있는지**를
실측으로 검증한 기록이다. 결론뿐 아니라 측정값과 함정을 남긴다.

## 실측 환경

| 항목 | 값 |
| --- | --- |
| OS | Windows 10.0.26300 (Windows 11) |
| 화면 | 2560x1440, 100% 배율 |
| 트레이 아이콘 크기 | `GetSystemMetrics(SM_CXSMICON)` = **16px** (정사각형) |
| 스택 | Rust 1.99 + `windows` 0.62 + `tray-icon` 0.26 |
| 상주 비용 | **10.6 MB**, 1초 갱신 시 단일 코어의 **0.15%** |

## 1. 텍스트를 트레이에 그리는 파이프라인

`Shell_NotifyIcon`은 HICON만 받는다. 텍스트 표시 API는 없다.
따라서 **매 갱신마다 비트맵을 그려 HICON을 교체**해야 한다.

구현한 파이프라인:

1. GDI 메모리 DC + 32bpp top-down DIB 섹션 생성
2. `Segoe UI` semi-bold로 8배 슈퍼샘플 크기로 흰 글자를 검은 배경에 렌더
3. 잉크(비영) 픽셀의 bbox를 잘라 박스 필터로 목표 크기까지 다운샘플
4. 흰색 RGB + 커버리지 알파로 RGBA 생성 → `tray_icon::Icon::from_rgba`

**함정**: `BI_RGB` 32bpp DIB에서 GDI는 B, G, R만 채우고 **예약 바이트는 0으로 둔다.**
알파 채널을 읽으려 하면 항상 0이라 아무것도 안 나온다. 휘도는 R 채널에서 읽어야 한다.

## 2. 정사각형 제약이 설계를 지배한다

아이콘이 정사각형이라 **폭에 맞추면 높이가 죽는다.** 후보 문자열별 실측 글리프 높이(16px 아이콘):

| 표시 문자열 | 글리프 높이 | 판정 |
| --- | --- | --- |
| `9` | 16px | |
| `12`, `63` | 12px | **읽힘** (작업 표시줄 시계 글자와 같은 크기) |
| `99` | 11px | 읽힘 |
| `1M` | 9px | 경계 |
| `12K`, `999` | 7px | 못 읽음 |
| `12%`, `1.2M` | 6px | 못 읽음 |
| `1.20M` | 4px | 못 읽음 |

**결론: 16px 아이콘에서 한 아이콘당 2글자가 실용 한계다.**
120% 이상 배율(24~32px)에서는 3~4글자도 가능하므로, 표기 폭은 아이콘 크기에 따라 자동으로 바꾸는 게 맞다.

가로로 눌러 높이를 살리는 방법(squeeze)도 시도했다. 높이는 항상 16px가 되지만
가로로 최대 4배 압축되어 획이 뭉갠다. 최후 수단으로만 쓸 만하다.

## 2-1. 라벨 + 값 2줄 레이아웃 실측

타입(CPU/MEM/NET)을 아이콘에 함께 보여주려면 정사각형 안에 2줄을 쌓아야 한다.
라벨 높이 / 값 높이 조합별 실측:

| 레이아웃 | 라벨 글리프 | 값 글리프 | 판정 |
| --- | --- | --- | --- |
| 값만 (`80%`) | - | 6px | 값이 작다 |
| 라벨 5 + 값 16 uniform | 5px | 6px | 값이 작다 |
| **라벨 5 + 값 9 squeeze** | 5px | **9px** | **채택** |
| 라벨 4 + 값 11 squeeze | 4px | 11px | 라벨이 4px로 읽히지 않는다 |
| 라벨 6 + 값 8 squeeze | 6px | 8px | 라벨은 낫지만 값이 작아진다 |

핵심은 **값을 가로 압축(Squeeze)하면 높이가 살아난다**는 점이다.
`80%`는 정사각형에 비율 유지로 맞추면 6px이지만, 높이 9px를 강제하고 가로를 0.67배로
압축하면 9px 높이를 얻는다. 0.67배 정도의 압축은 획이 뭉개지지 않는다
(정사각형에 4글자를 비율 유지로 넣을 때의 4배 압축과는 질이 다르다).

시트 생성: `python tools/icon_sheet.py out/sheet.bmp <레이아웃 이름들>`
(행 = 레이아웃, 열 = CPU / MEM / NET, 8배 확대)

## 3. 새 아이콘은 기본적으로 오버플로우에 숨는다

첫 실행에서 아이콘 3개가 트레이에 보이지 않았다. 레지스트리로 확인:

```
HKCU\Control Panel\NotifyIconSettings\<hash>\IsPromoted
```

처음에는 값이 **없었고**(= 숨김), `0x1`이 된 뒤에야 트레이에 보였다.
즉 사용자가 한 번 승격(설정 → 작업 표시줄 → 기타 시스템 트레이 아이콘, 또는 오버플로우에서 드래그)해야 한다.

**설치 직후에는 아무것도 안 보이는 상태가 기본이다.** 한동안은 첫 실행 안내로 작업
표시줄 설정을 열어 주었지만, 지금은 앱이 승격 상태를 읽지도 안내하지도 않는다
(§15 「승격 감지 제거」). 고정할지는 사용자가 정한다.

## 4. 아이콘 식별 키는 실행 경로에 묶인다 (툴팁은 무관)

관찰한 키 구성: 실행 경로당 3개(CPU/MEM/NET) = 아이콘 ID 단위.

실험으로 확인한 것:

- 등록 시 툴팁을 `CPU` → `CPU_vY`로 바꿔도 **새 키가 생기지 않는다** (기존 키 유지)
- 런타임에 툴팁을 매초 바꿔도 새 키가 생기지 않는다
- 앱을 재시작해도 키가 그대로다

**따라서 승격 설정은 툴팁 변경과 재시작에 안전하다.** 단 **실행 경로가 바뀌면 초기화된다.**
자동 업데이트를 `app-1.2.0/app.exe` 같은 버전별 경로로 설계하면 승격 설정이 매번 날아간다.
exe 경로를 고정하는 업데이트 방식(제자리 교체)을 써야 한다.

## 5. 밟은 함정 (재발 방지용)

| 함정 | 증상 | 대응 |
| --- | --- | --- |
| 빈 문자열을 GDI에 전달 | **프로세스 세그폴트** (빈 슬라이스의 dangling 포인터를 GDI가 건드림) | 렌더 전에 빈 문자열 가드 |
| `SetTimer(hwnd=NULL, ...)` | 요청한 ID를 무시하고 Windows가 자체 ID를 발급 (예: 16573) | 반환값으로 비교 |
| 알파 채널 읽기 | DIB의 예약 바이트는 항상 0 → 빈 아이콘 | R 채널(휘도)을 알파로 사용 |

## 6. 성능

1Hz 갱신, 아이콘 3개 기준.

| 구성 | 단일 코어 사용률 | 메모리 |
| --- | --- | --- |
| 매 갱신마다 GDI 폰트/DC 생성 | 0.32 ~ 0.46% | 10.6 MB |
| 값 문자열 렌더 캐시 적용 | **0.15%** | 10.6 MB |

(메모리는 시작 직후의 작업집합이다. 정상 상태에서는 [PLAN.md](PLAN.md) 참고.)

값 문자열은 CPU/메모리 0~99, 네트워크 단위 조합처럼 **유한 집합**이므로
문자열 단위 캐시만으로 비용이 절반 이하로 떨어진다. 실용적으로는 무시할 수 있는 수준이다.

> 위 수치는 아이콘당 글리프가 **1개**일 때(라벨 없이 값만) 측정한 값이다.
> 라벨을 붙이면 글리프가 2배가 되어 출시 버전은 18.7 MB / **0.41%** 이다.
> 전체 수치는 [PLAN.md](PLAN.md) 참고.

## 7. 결론

채택: **Rust + `tray-icon` + `windows-rs`, 지표는 무권한 Win32 API**
(`GetSystemTimes`, `GlobalMemoryStatusEx`, `GetIfTable2`).

- 1초 갱신, 3개 아이콘, 10 MB 남짓 — 상주 앱으로 충분히 가볍다
- 표현력의 상한은 "아이콘당 2글자"라는 점을 제품 컨셉에 반영해야 한다
- 3글자 이상이 필요해지면 대안은 다음 절

## 8. 검증 도구

| 도구 | 용도 |
| --- | --- |
| `cargo run -- --dump` | 레이아웃별 글리프 높이 측정 + 아이콘 BMP 덤프 |
| `tools/icon_sheet.py` | 덤프한 아이콘을 레이아웃 비교 시트로 합침 |
| `tools/crop_png.py` | 스크린샷에서 트레이 영역을 잘라 확대 |
| `tools/traykeys.py` | `NotifyIconSettings`에서 아이콘 키와 승격 상태 조회 |
| `tools/traykeys_cleanup.py` | 이 앱이 등록한 트레이 아이콘 키 삭제 |

스파이크 소스는 `~/Workspaces/winetree94/tray-monitor`에 있다.
BMP를 읽는 도구들은 BMP가 bottom-up이라는 점을 반영해 행 순서를 뒤집는다
(이걸 빼먹으면 비교 시트가 상하 반전된다).

## 9. 아직 검토하지 않은 대안

| 대안 | 언제 필요한가 | 위험 |
| --- | --- | --- |
| 태스크바 임베드 (`SetParent(Shell_TrayWnd)`) | 3글자 이상, 그래프, 자유로운 폰트가 필요할 때 (TrafficMonitor 방식) | 비공식 API, Windows 업데이트로 깨질 수 있음 |
| 아이콘 1개 + 플라이아웃 패널 | 상세 그래프를 팝업에서 보여줄 때 | 트레이에는 숫자가 안 보임 |
| 온도/팬/전압 지표 | LibreHardwareMonitor / HWiNFO 연동 | 관리자 권한 + 커널 드라이버 필요 |
| OpenGL/Direct2D 렌더링 | 아이콘에 그래프를 그릴 때 | 현재 요구에는 과함 |

## 10. 리소스 파이프라인과 매니페스트 (설정 GUI G0)

설정 GUI를 Win32 대화상자로 만들기로 했고([PLAN.md](PLAN.md) §12), 그 전제인 `rc.exe` ·
매니페스트 · DPI 선언을 먼저 실측했다. 환경은 Windows 11 26H2 (build 26300), MSVC 툴체인
(호스트 `x86_64-pc-windows-msvc`), Windows Kits 10.0.26100.0.

### 10-1. rc.exe에는 기본 include 경로가 없다

`rc.exe`는 PATH에 없다(`C:\Program Files (x86)\Windows Kits\10\bin\<sdk>\x64\rc.exe`).
`build.rs`는 탐색을 `embed-resource`에 맡긴다.

**함정**: `rc.exe`를 직접 호출하면 `#include <windows.h>`에서 죽는다.

```
probe.rc(1) : fatal error RC1015: cannot open include file 'windows.h'
```

VS 개발자 프롬프트 밖에서는 `%INCLUDE%`가 비어 있고 rc.exe에 기본 경로가 없다.
SDK의 `Include\<version>\{um,shared}`를 `/I`로 넘기면 컴파일된다(실측).
`embed-resource`는 Windows Kits의 `Include\<version>\*`를 `%INCLUDE%`에 채워 주므로
`build.rs`에서 경로를 다룰 필요가 없다. 리소스 컴파일을 직접 스크립트로 부르지 않는 이유다.

### 10-2. 리소스가 exe에 들어갔는지 확인하는 방법

| 대상 | 확인 명령 |
| --- | --- |
| `VERSIONINFO` | `[System.Diagnostics.FileVersionInfo]::GetVersionInfo(exe)` |
| 매니페스트 | `mt.exe -inputresource:tray-monitor.exe;#1 -out:manifest.xml` |
| comctl32 v6 활성 | 실행 중 프로세스의 로드된 모듈 경로 |
| DPI 선언 적용 | 앱 로그 |

`VERSIONINFO` 실측값: `FileVersion 0.2.0`, `ProductVersion 0.2.0`, `FileDescription`,
`CompanyName`, `LegalCopyright`, `OriginalFilename`, `ProductName` — 모두 `Cargo.toml`과
`build.rs`가 만든 `version_defines.rc`에서 온 값이다.

**함정**: comctl32 v6가 활성화되어 있어도 로드된 `comctl32.dll`의 **파일 버전 문자열은
`5.82 (WinBuild.160101.0800)`로 나온다.** 판별은 경로로 해야 한다.

```
C:\Windows\WinSxS\amd64_microsoft.windows.common-controls_6595b64144ccf1df_6.0.26100.9711_none_...\comctl32.dll
```

SxS 어셈블리 이름의 `6.0.26100.9711`이 v6의 증거다. 파일 버전만 보고 "v6가 안 붙었다"고
판단하면 멀쩡한 매니페스트를 고치게 된다.

### 10-3. DPI 선언은 매니페스트와 런타임 호출 중 하나만 살아 있다

`resources/app.manifest`가 `PerMonitorV2`를 선언하면 `SetProcessDpiAwarenessContext(...)`는
"이미 설정됨"으로 실패한다. 그래서 앱은 그 반환값을 로그로 남긴다.

```
DPI: 매니페스트의 PerMonitorV2 선언을 사용한다
```

이 줄이 나오면 매니페스트가 실제로 링크된 것이고, `DPI: 런타임 호출로 PerMonitorV2를
설정했다(매니페스트 없음)`이 나오면 리소스가 빠진 빌드다. 리소스는 링크 단계의 문제라
컴파일 성공만으로는 확인할 수 없다.

### 10-4. 비용

| 항목 | 값 |
| --- | --- |
| exe | 505,344 → **508,928 bytes** (+3,584) |
| `Cargo.lock` 패키지 | 131 → **138** (`cc`, `embed-resource`, `find-msvc-tools`, `shlex`, `vswhom`, `vswhom-sys`, `winreg`) |
| 런타임 의존성 | **변화 없음** (전부 build-dependency) |
| 로컬 크레이트 재빌드 | 12초 (리소스 컴파일 포함) |
| 품질 검사 | `fmt` · `clippy -D warnings` · `test` 97개 · `--dump` BMP 36개 그대로 통과 |

### 10-5. 아직 재지 않은 것

- **PMv2의 대화상자 자동 스케일**: G0에는 대화상자가 없다. 100 / 150 / 200% 모니터를 오가며
  창 크기와 컨트롤 배치를 재는 일은 G3의 실제 설정 창에서 한다. 지금 확인한 것은 매니페스트의
  DPI 선언이 실제로 적용된다는 것까지다.
- **비주얼 스타일이 적용된 컨트롤의 모양**: 컨트롤을 만드는 코드가 G3에 있다.
- **아이콘 리소스**: 디자인 미정이라 G8로 미뤘다.

## 11. 설정 창 검증 (설정 GUI G1 ~ G7)

Win32 대화상자로 만든 설정 창을 띄워 컨트롤을 조작하며 확인한 기록이다. 사람이 하는 것과
같은 경로(cua-driver로 실제 클릭·입력)로 했다.

### 11-1. 창 크기

템플릿은 248 × 188 DLU다. 96 DPI(100% 배율)에서 450 × 392 px로 떴다(가로 450 px ÷ 248 DLU
≈ 1.8 px/DLU). PMv2라 배율이 다른 모니터로 옮기면 시스템이 같은 비율로 다시 그린다.

### 11-2. 확인한 동작

| 항목 | 결과 |
| --- | --- |
| 체크박스 4종 | 끄면 트레이 아이콘이 즉시 사라지고 `config.toml`에 반영 |
| 마지막 지표 규칙 | 마지막 하나를 끄려 하면 체크가 되돌아오고 상태줄에 `the last readout cannot be turned off` |
| 주기 프리셋 | 콤보에서 250 ms 선택 → `폴링 주기 = 250ms`, 하트비트 간격도 15초로 바뀜 |
| 주기 직접 입력 | 입력 칸에 750 + 포커스 이동 → `poll_interval_ms = 750`, 콤보가 `Custom…`으로 바뀜 |
| 범위 보정 | 100 입력 → 250으로 보정하고 상태줄에 이유를 표시 |
| 색 모드 | 라디오 전환 즉시 미리보기와 트레이 아이콘 색이 바뀜 |
| 가상 어댑터 | `exclude_virtual_adapters = false` |
| 자동 실행 | `HKCU\...\Run`에 현재 경로가 들어가고, 해제하면 사라짐 |
| 기본값 복원 | 바꾼 항목만 되돌리고 `config.toml`이 기본값으로 돌아감 |
| 닫기 | 창이 닫히고 앱은 계속 돈다 |
| 창이 열린 동안 | 하트비트가 계속 찍힌다 — 모달 루프에서도 타이머가 산다 |
| 두 번째 실행 | 실행 중 인스턴스에 설정 창이 뜨고 두 번째 프로세스는 조용히 끝난다 |

### 11-3. 미리보기 비트맵

정적 컨트롤(`SS_BITMAP`)은 알파를 합성하지 않고 비트를 그대로 복사한다. 그래서 미리보기는
아이콘을 **작업 표시줄 배경색 위에 합성한 불투명 픽셀**로 만든다. `STM_SETIMAGE`가 돌려주는
이전 핸들을 지우지 않으면 갱신할 때마다 GDI 핸들이 샌다(`win32::dialog::set_bitmap`이 그
자리에서 지운다).

### 11-4. 검증 중에 찾은 결함 (계획에 없던 것)

1. **자동 실행 해제가 `Access is denied`로 실패했다.** `windows_registry::CURRENT_USER.open()`은
   읽기 전용 키를 주므로 `remove_value`가 거부된다. 해제할 때도 `create()`로 쓰기 권한을
   얻어야 한다. 트레이 메뉴의 같은 토글도 v0.2.0부터 이 상태였다 — 등록은 되고 해제는 실패했다.
2. **실패를 알리는 알림창이 트레이를 얼렸다.** 알림창이 뜬 동안 `App` 대여가 풀리지 않아
   타이머 틱이 전부 버려졌다(하트비트가 멈춘 것으로 확인). 반영 실패를 `apply_effects`의
   반환값으로 올려 부른 쪽이 알리도록 바꿨다 — 설정 창은 상태줄, 트레이 메뉴는 알림창이다.
3. **다른 프로세스에서 `GetWindowText`/`SetDlgItemText`로 입력 칸을 읽고 쓰면 안 된다.**
   Windows는 다른 프로세스의 컨트롤 텍스트를 캐시된 값으로 돌려주고, 그 상태에서 보낸 쓰기는
   반영되지 않는다. 검증은 UIA(`set_value`)나 실제 입력으로 해야 한다. 이 함정 때문에 처음에는
   "입력 칸이 동작하지 않는다"고 잘못 판단했다.
4. **설정 창이 열려 있으면 트레이 메뉴 클릭이 밀렸다.** 메뉴 이벤트를 `MenuEvent::receiver()`
   채널로 받아 메인 루프에서 비웠는데, 설정 창의 모달 루프가 도는 동안 메인 루프는 멈춘다.
   그래서 지표 체크를 눌러도 반영되지 않다가 창을 닫는 순간 한꺼번에 반영됐다(로그에서
   `settings window closed`와 같은 초에 `show cpu`가 몰려 찍힘). `MenuEvent::set_event_handler`로
   클릭을 그 자리에서 처리하고(`runtime::menu_event`), `App`을 빌릴 수 없을 때만 큐에 남겨
   다음 메시지·타이머 틱에서 비운다. 열려 있는 설정 창에는 `WM_APP_REFRESH`를 보내 체크박스와
   미리보기도 맞춘다.
5. **`Settings…`를 두 번 누르면 창을 닫을 때 한 번 더 떴다.** 설정 창이 열린 동안 들어온 요청이
   "열기 예약" 플래그만 세웠고, 창이 닫히면 메시지 루프가 그 플래그를 보고 다시 열었다.
   요청은 이제 열린 창이 있으면 그 창을 앞으로 가져오고 끝낸다(`settings::dialog::focus`).
   두 번째 실행은 IPC 메시지를 보내기 전에 `AllowSetForegroundWindow`로 실행 중인 인스턴스에
   포그라운드 권한을 넘긴다. 백그라운드 프로세스의 `SetForegroundWindow`는 거부되기 때문이다.

### 11-5. 다크 모드 (uxtheme 비공식 API)

| 항목 | 결과 |
| --- | --- |
| ordinal 133 `AllowDarkModeForWindow` | 있음 |
| ordinal 135 `SetPreferredAppMode` | 있음 |
| ordinal 136 `FlushMenuThemes` | 있음 |
| 결과 | 창 배경·라벨·체크박스·라디오·입력 칸·버튼이 모두 어두워진다. 미리보기는 작업 표시줄 배경색 위에 합성되므로 대화상자 배경이 같은 색일 때 아이콘만 떠 있는 것처럼 보인다 |

**함정: 테마를 입히면 테마 메시지가 다시 온다.** `SetPreferredAppMode`와 `SetWindowTheme`은
`WM_SETTINGCHANGE`/`WM_THEMECHANGED`를 다시 보낸다. 그 메시지에서 다시 테마를 입히면 재진입이
끝없이 쌓여 **스택 오버플로(`c00000fd`, ntdll)로 프로세스가 죽는다**(build 26300에서 실측).
스레드 지역 플래그로 한 번만 처리하게 했다.

**함정: `DarkMode_Explorer`는 라디오 글자색이 없다.** `OpenThemeData`로 확인하면
`DarkMode_Explorer::Button`은 체크박스(`BP_CHECKBOX`)의 `TMT_TEXTCOLOR`만 흰색으로 정의하고
라디오(`BP_RADIOBUTTON`)는 `E_NOTFOUND`다. 그래서 `Colours`의 라디오 두 개만 어두운 배경 위에
검정 글자로 그려졌다(build 26300 실측). `DarkMode_DarkTheme::Button`은 둘 다 흰색이라
라디오에는 이 테마를 입히고, 둘 다 없는 빌드에서는 비주얼 스타일을 꺼서
`WM_CTLCOLORSTATIC` 글자색이 적용되게 한다(`win32::dark::paint_radio`).

`--theme`로 지금 상태를 볼 수 있다.

```
taskbar theme : dark
dark mode API : available (uxtheme ordinals 133/135/136)
app mode      : ForceDark
```

---

## 12. 1비트 픽셀 폰트 (v0.4.0)

기존 렌더(GDI 8배 슈퍼샘플 -> 박스 다운샘플)은 16px에서 라벨 5px를 AA 회색으로
뭉갰다. 대안을 실측하고 1비트 픽셀 폰트로 교체했다. 계획은
[PIXEL-FONT.md](PIXEL-FONT.md), 구현은 PLAN.md §14.

### 대안 측정: GDI 네이티브 비AA

`NONANTIALIASED_QUALITY`로 목표 크기에 바로 그리면 hinting를 GDI가 해 주므로
유지보수가 가장 쉽다.

| 폰트 | px | 계조 | `CPU` 높이 | `80%` 폭 |
| --- | --- | --- | --- | --- |
| Segoe UI | 16 | 2 (1비트) | 11px | 19px |
| Tahoma | 16 | 2 | 12px | 21px |
| Small Fonts | 16 | 2 | 12px | 18px |
| Terminal | 16 | 2 | 12px | 19px |

**1비트는 나오지만 폭이 문제다.** 5px 라벨은 모든 폰트에서 획이 끊겼고, 9px 값은
18~20px로 16px 아이콘을 넘는다. `1.2M`을 16px 안에 넣으려면 글리프가 3px 폭이어야
하는데 시스템 폰트는 5~6px다. 그래서 직접 만든 좁은 글리프를 임베드한다.

### 비정수 배율의 한계

16px master를 각 크기로 nearest-neighbor 확대했을 때의 가로 run 길이:

| px | 배율 | run 길이 | 판정 |
| --- | --- | --- | --- |
| 16 | 1x | 1, 2, 3 | 균일 |
| 20 | 1.25x | 1, 2, 3, 4 | 획 ±1px |
| 24 | 1.5x | 1, 2, 3, 5 | 획 ±1px |
| 28 | 1.75x | 1, 2, 3, 4, 5 | 획 ±1px |
| 32 | 2x | 2, 4, 6 | 균일 |

1.25px 두께의 균일한 획은 1비트 이미지에 존재하지 않는다. 구현이 아니라 수학이라,
비정수 배율에서는 ±1px을 감수한다(모든 배율에서 계조는 `[190, 255]`로 1비트 유지).

### 채택한 구성

> 아래는 v0.4.0 당시 구성이다. 현재 구성(캡션 3x5 + 값 4x10, 토글 없음)은
> [PIXEL-FONT.md](PIXEL-FONT.md) §9.

| 항목 | 값 |
| --- | --- |
| 라벨 | 3x5 (글리프 폭 3px) |
| 값 | 3x9, `%`만 4px, `.` 1px |
| 값(단위 표시, 라벨 숨김) | 4x10 (`VALUE_LARGE`), 간격 0 |
| 값(단위 숨김, 라벨 숨김) | 4x13 (`VALUE_HUGE`), 간격 1 |
| 간격 | 1px |
| master | 16px, 라벨 블록 15px, 값만 블록 10px, 단위까지 숨기면 13px |
| 폭 예산 | `100%`/`999K` = 15px, `1.2M` = 13px, 단위 없이 `100`/`1.2` = 14px |
| 계조 | 라벨 190, 값 255 (회색 없음) |
| 글리프 | 라벨 8자 + 값 17자 + 큰 값 17자 + 확대 값 12자 |

---

## 13. 다국어 UI와 설정 레이아웃 (v0.7.0)

설정 창을 그룹 + 설명 구조로 다시 짜고 영어/한국어/일본어 UI를 넣기 전에, 세 가지를
실측했다: 런타임 텍스트 주입과 실시간 언어 전환, `Segoe UI`의 CJK 폴백, 창 크기.
계획은 PLAN.md §15.

### 13-1. 런타임 텍스트 주입과 실시간 전환

`.rc`의 `IDD_SETTINGS`는 글자를 모두 비워 두고, `WM_INITDIALOG`에서 `SetDlgItemTextW`로
`src/i18n.rs`의 표를 채운다. `DIALOGEX`는 `WS_VISIBLE` 없이 만들어지므로 첫 페인트 전에
채워지고, 빈 글자가 깜빡이지 않는다.

언어 콤보로 바꾸면 창을 닫지 않고 그 자리에서 바뀐다. UIA 트리로 확인한 결과:

| 단계 | 창 제목 | 지표 체크박스 | 주기 콤보 | 버튼 |
| --- | --- | --- | --- | --- |
| 시작(시스템 영어) | `TinyTrayManager settings` | `CPU` / `Memory` / `Network down` / `Network up` | `1 second` | `Reset to defaults` / `Close` |
| 日本語 선택 | `TinyTrayManager の設定` | `CPU` / `メモリ` / `ネットワーク下り` / `ネットワーク上り` | `1 秒` | `既定値に戻す` / `閉じる` |
| 한국어 선택 | `TinyTrayManager 설정` | `CPU` / `메모리` / `네트워크 다운로드` / `네트워크 업로드` | `1초` | `기본값으로 복원` / `닫기` |

주기 콤보는 `CB_RESETCONTENT` 후 프리셋 + `Custom…`을 지금 언어로 다시 채우고 선택을
복원한다(콤보 항목 자체가 번역 대상이다). 창 제목은 `SetWindowTextW`로 바꾼다.

선택은 `config.toml`에 선택값 그대로 저장된다(`language = "ko"`). 프로세스를 다시 띄우면
창이 한국어로 열린다. `Auto`는 `GetUserDefaultUILanguage`의 LANGID 하위 10비트를 보고
`LANG_KOREAN`/`LANG_JAPANESE`만 골라 쓰고 나머지는 영어로 떨어진다.

### 13-2. CJK 폰트 링킹

대화상자 `FONT`는 `Segoe UI` 9pt 그대로 둔다. Segoe UI에는 한글·일본어 글리프가 없지만
Windows 폰트 링킹이 맑은 고딕 / Yu Gothic으로 대체해 준다. 한국어·일본어 창을 캡처해
확인했고, 모든 라벨이 읽힌다. 폰트를 바꾸는 폴백(`Malgun Gothic`/`Yu Gothic UI`)은
필요 없다.

### 13-3. 창 크기

| 항목 | 값 |
| --- | --- |
| `DIALOGEX` 단위 | 300 x 297 DLU (v0.6.0은 248 x 188) |
| 실측 픽셀(창) | **541 x 596 px** (100% 배율) |
| 실측 픽셀(클라이언트) | 525 x 557 px |
| 세로 DLU -> px | 약 2.01 (297 DLU -> 596 px) |

「표시 항목」 그룹에 안내 한 줄(`IDC_HINT_TASKBAR`)을, 바닥에 「작업 표시줄 설정 열기」
버튼(`IDC_OPEN_TASKBAR`)을 추가하면서 288 -> 297 DLU가 됐다. 높이가 596px라 1080p 200%
배율에서는 작업 영역(약 1030px)을 넘고, 175%도 1043px로 넘친다. 100/125/150%에서는 들어간다.
그룹 4개 + 설명 5줄 + 미리보기 + 버튼이 차지하는 최소치이고, 이보다 줄이려면 그룹이나
미리보기를 빼야 한다. 고배율은 1440p 이상에서 주로 쓰므로 그대로 둔다.

### 13-4. 트레이 메뉴

`muda`의 `CheckMenuItem`/`MenuItem`/`PredefinedMenuItem` 모두 `set_text`가 있어 아이콘을
재생성하지 않고 글자만 바꾼다. 재생성하면 Windows가 부여한 아이콘 id와 승격 설정이
어긋나므로(AGENTS.md 함정 1·17) 반드시 `set_text`를 쓴다. 툴팁은 다음 틱의 값 비교에
걸려 갱신된다.

### 13-5. 검증 도구의 함정

UIA `ValuePattern.SetValue`로 언어 콤보를 바꾸면 값은 바뀌지만 `CBN_SELCHANGE`가 오지
않아 창이 따라오지 않는다. 콤보를 `expand`하고 목록의 `ListItem`을 눌러야 실제 클릭과
같은 알림이 온다. 자동화로 언어 전환을 검증할 때 이 경로를 써야 한다.

### 13-6. 그룹 박스 캡션 색 (v0.7.1)

그룹 + 설명 구조로 바꾸자 다크 모드에서 그룹 제목(`Readouts` 등)이 **검정 글자로** 그려져
보이지 않았다. 원인은 라디오 버튼과 같다(§11-5). `DarkMode_Explorer::Button`은 체크박스
글자색만 정의하고 그룹 박스(BS_GROUPBOX) 캡션 색은 정의하지 않는다.

`win32::dark::paint_group_box`가 그룹 박스의 비주얼 스타일을 끄면, 클래식 그룹 박스가
`WM_CTLCOLORSTATIC`의 글자색(`TEXT` = `0xF0F0F0`)으로 캡션을 그려 보인다. 프레임도 클래식
(3D 에칭)으로 바뀌는데, 어두운 배경에서 밝은 선이라 오히려 잘 보인다. 라이트 모드는 이
경로를 타지 않아 그대로다.

---

## 14. GPU 사용률과 전용 VRAM (v0.8.0)

관리자 권한 없이 GPU 엔진 사용률과 전용 VRAM 상주량을 읽는다. 계획과 구현은
[PLAN.md](PLAN.md) §16.

### 수집 경로 두 개

| 값 | 호출 | 비고 |
| --- | --- | --- |
| 어댑터 목록·전용 한도 | `CreateDXGIFactory1` → `EnumAdapters1` | `DXGI_ADAPTER_FLAG_SOFTWARE`와 `IDXGIAdapter3`를 못 얻는 어댑터를 뺀다 |
| 엔진별 누적 실행 시간 | `gdi32!D3DKMTQueryStatistics` (`QUERYSTATISTICS_NODE` = 5) | `NODE_INFORMATION.GlobalInformation.RunningTime` |
| 세그먼트별 상주 바이트 | 같은 함수 (`QUERYSTATISTICS_SEGMENT` = 3) | `SEGMENT_INFORMATION.BytesResident`와 `Aperture` |

`D3DKMTQueryStatistics`는 공개 헤더에 있지만 `windows` 크레이트에 정의가 없어
`gdi32.dll`에서 함수 포인터를 직접 얻고 구조체를 손으로 적었다.

### 구조체 배치 (`d3dkmthk.h`, SDK 10.0.26100.0)

| 오프셋 | 필드 | 근거 |
| --- | --- | --- |
| 0 | `Type` (u32) | |
| 4 | `AdapterLuid` (`LUID`) | LUID는 4바이트 정렬 |
| 16 | `hProcess` | 포인터는 8바이트 정렬 → 12~16이 패딩 |
| 24 | `QueryResult` | 776바이트 |
| 800 | `QueryId` union | `SegmentId`/`NodeId`가 union의 첫 4바이트 |
| 전체 | 808바이트 | 헤더의 `C_ASSERT(sizeof(D3DKMT_QUERYSTATISTICS) == 0x328)` (x64 전용) |

`QueryResult` 안쪽 오프셋도 같은 헤더에서 읽었다.

| 구조체 | 필드(오프셋) |
| --- | --- |
| `ADAPTER_INFORMATION` | `NbSegments`(0), `NodeCount`(4) |
| `NODE_INFORMATION.GlobalInformation` | `RunningTime`(0) |
| `SEGMENT_INFORMATION` | `CommitLimit`(0), `BytesCommitted`(8), `BytesResident`(16), `Aperture`(40) |

`kmt_statistics_layout_matches_the_wdk` 테스트가 `size_of`와 `offset_of`로 이 배치를
고정한다. 배치를 손으로 적은 이상, 조용히 어긋나는 것보다 테스트로 깨지는 편이 낫다.

### 단위: 주석은 micro-second, 실측은 100ns

헤더는 `RunningTime` 주석에 "Running time in micro-second"라고 적었다. µs로 나누면
사용률이 10분의 1이 된다. NVENC를 4K60으로 포화시켜 `nvidia-smi`와 교차 검증했다.

| 구간 | `nvidia-smi utilization.encoder` | 아이콘 값 |
| --- | --- | --- |
| 부하 | 99%, 99%, 99%, 99%, 99% | 91.4%, 99.8%, 99.3%, 99.9%, 99.8% |
| 해제 후 | 7%, 5%, 1% | 5.6%, 6.6%, 6.0% |

100ns로 나눠야 인코더 포화가 100%로 나온다. 테스트 `percent_uses_100ns_ticks`가
이 단위를 고정한다.

### VRAM: aperture가 아닌 세그먼트의 합

`Aperture`가 0인 세그먼트(로컬 전용 메모리)의 `BytesResident`를 모두 더하고, DXGI가
알려주는 전용 한도로 나눈다. PDH `GPU Adapter Memory(*)\Dedicated Usage`와 같은 구간에서
비교했다(한도 7,956MB).

| 아이콘 | 아이콘 환산 | PDH Dedicated Usage |
| --- | --- | --- |
| 87.5% | 6,962 MB | 6,958 MB, 6,966 MB |
| 79.8% | 6,350 MB | 6,349 MB |

정의가 다른 값도 있다. `nvidia-smi memory.used`는 6,337 MiB였고, 전용 한도도 8,188 MiB로
DXGI(7,956MB)보다 크다(예약분 포함). 작업 표시줄에 보여줄 값은 Windows가 보고하는 쪽,
즉 DXGI 한도와 `BytesResident` 합으로 정했다.

### PDH `GPU Engine`은 쓰지 않는다

처음에는 PDH 카운터를 기준으로 삼으려 했다. 이 기기에서는 값이 갱신되지 않았다. NVENC를
포화시킨 10초 동안 `GPU Engine(*engtype_videoencode)`가 11%에 머물렀고, 같은 구간에
`nvidia-smi`는 99%, `D3DKMTQueryStatistics`도 99%를 보고했다. `Get-Counter`가
"The data in one of the performance counter samples is not valid"로 실패하기도 했다.
수집 경로는 `D3DKMTQueryStatistics` 하나로 간다.

### 사용률은 엔진별 최대값

노드는 엔진 하나(3D·복사·비디오 인코더…)에 대응한다. 아이콘 하나로 GPU 전체를 요약하려면
엔진별 값 중 최대를 쓰는 게 가장 설명하기 쉽다. NVENC 포화 시 3D는 18~27%, 인코더는 99%를
가리켰고 아이콘은 99%를 보여줬다.

엔진별 그래프를 나눠 보여주는 도구와 비교할 때는 "가장 바쁜 엔진" 기준임을 기억해야 한다.
엔진 사용률을 모두 더한 값(예: 3D 15% + 복사 7% = 22%)과는 다르다.

### 아직 재지 않은 것

- 어댑터 선택 기준: 지금은 소프트웨어 렌더러를 뺀 뒤 전용 메모리가 가장 큰 어댑터다.
  하이브리드 노트북에서 화면을 그리는 GPU가 아닐 수 있다.
- Intel/AMD의 세그먼트 구성: `Aperture == 0` 필터가 전용 메모리만 고르는지 확인하지 않았다.
- 절전 복귀 후 첫 표본: `reset()`이 기준점을 다시 잡지만 실측하지는 않았다.
- 전용 메모리가 0인 어댑터: CI 러너(`windows-2025`)에는 실제 GPU가 없어 "Microsoft Basic Render Driver"(WARP)가 잡히고 전용 한도가 0MB다. 이때 VRAM 사용률은 0%로 두고, 스모크 테스트가 그 경우를 허용한다(로컬에서는 RTX 4060 7,956MB로만 확인).

---

## 15. 트레이 아이콘 정체성과 `NotifyIconSettings` (v0.8.0)

승격(작업 표시줄에 고정)이 어느 키에 저장되는지, 그리고 셸이 키를 언제 쓰는지를 앱 밖에서
직접 재현했다. `Shell_NotifyIconW`를 호출하는 임시 PowerShell 프로브로 `NIM_ADD`/
`NIM_MODIFY`의 반환값과 레지스트리 쓰기를 함께 관측했다.

### 실측

| 실험 | `NIM_ADD` | `NIM_MODIFY`(NIF_GUID) | `NotifyIconSettings` |
| --- | --- | --- | --- |
| 처음 보는 GUID | true | true | 키 생성(관측 즉시) |
| 같은 GUID 재등록(삭제 후) | true | true | 같은 키가 다시 생성 |
| 기존 `TRAY_GUIDS`(CPU)로 등록 | true | true | **키가 15초 넘게 안 생김** |
| 경로 기반, uID=1, 툴팁 A | true | - | 키 생성(`UID=0x1`) |
| 경로 기반, uID=2, 같은 툴팁 A | true | - | **다른 키** 생성(`UID=0x2`) |
| 경로 기반, uID=1, 다른 툴팁 B | true | - | 새 키 없음, 기존 키의 `InitialTooltip`은 A 그대로 |

### 정체성 규칙

1. **경로 기반 정체성은 `(ExecutablePath, uID)`다.** 툴팁은 키가 아니라 키에 기록되는 값이고
   (`InitialTooltip`), 최초 등록 시점 값에서 갱신되지 않는다. 그래서 툴팁을 바꿔도 고정은
   같은 아이콘에 남고, uID가 밀리면 다른 아이콘으로 옮겨간다.
2. **셸은 이미 아는 정체성에는 키를 (다시) 쓰지 않는다.** 키가 없다는 것은 거부가 아니라
   "이 Explorer 세션이 그 정체성을 이미 본 적이 있다"는 뜻이다.
3. GUID로 등록하면 정체성이 GUID가 되어 uID·경로와 무관해진다.

### 고친 결함

`Tray::build`는 `NotifyIconSettings`에 키가 생기는지를 GUID 수락 판정으로 썼다. 규칙 2 때문에
**두 번째 실행부터 항상 폴백**했고, 폴백은 GUID 시도가 쓴 uID(`1..N`) 뒤에서 다시 만들어져
`N+1..2N`을 받았다. 지표를 4개에서 6개로 늘리면 경로 기반 정체성이 통째로 밀려 기존 고정이
다른 아이콘으로 가거나 사라진다(AGENTS.md 규칙 1·23이 경고하는 상황).

수정: 판정은 `NIM_ADD` 성공과 `win32::set_guid_tooltip`(`NIF_TIP | NIF_GUID`)의 성공만 본다.

### 수정 후 검증 (Explorer 재시작 → 앱 실행)

| 단계 | 결과 |
| --- | --- |
| 시작 로그 | 폴백 경고 없음 |
| 레지스트리 | 6개 키 전부 `IconGuid` 기록, `ExecutablePath`는 우리 exe |
| `tools/promote.py`로 4개 고정 | 작업 표시줄에 4개 표시 |
| exe를 다른 폴더로 복사해 실행 | 같은 6개 키가 새 경로로 이동, `IsPromoted=1` 유지(MSIX 업데이트 시나리오) |

### 승격 감지 제거

이 실측을 마친 뒤, 앱이 승격 상태를 읽는 것 자체를 걷어냈다. 트레이에 무엇을 고정할지는
사용자의 결정이고, 앱이 그 상태를 읽어 안내하거나 되묻는 동작은 필요하지 않다.

| 항목 | 이전 | 이후 |
| --- | --- | --- |
| 승격 판정 | `onboard::promotion_state`가 `NotifyIconSettings`를 순회하며 `IsPromoted` 집계 | 없음 |
| 진단 | `onboard::guids_registered`가 `IconGuid` 키 개수를 경고로 남김 | 없음 |
| 첫 실행 안내 | `Promotion`이 `All`이 아니면 `MB_YESNO` 후 작업 표시줄 설정을 엶 | 없음 |
| 상태 파일 | `state.toml`의 `onboarding_shown`/`last_version` | 파일 자체를 읽지 않음 |

남은 것은 아이콘 등록뿐이다. 고정 GUID 등록과 수락 판정(위 「고친 결함」)은 그대로
유지된다: 경로 기반 정체성이 밀리면 사용자가 고정해 둔 아이콘이 다른 지표로 옮겨가므로,
GUID 등록은 승격을 감지하기 위해서가 아니라 사용자의 선택을 보존하기 위해 필요하다.

### 함정

- **실행 중인 앱을 `/F`로 죽이지 않는다.** 죽은 창의 정체성이 Explorer 세션에 남아, 같은
  GUID·경로로 다시 등록해도 키가 기록되지 않는다(관측). 그 상태에서는 Explorer를 재시작해야
  한다. 정상 종료는 `Exit` 메뉴나 IPC 창의 `WM_CLOSE`(installer가 쓰는 경로)다.
- 셸이 승격을 기록할 자리가 없는 상태(키 없음)에서는 사용자가 고정을 눌러도 기억되지 않는다.
  그때는 Explorer를 재시작한 뒤 앱을 다시 실행한다.

## 16. 실행 중 배율 변경과 트레이 아이콘 크기 (v0.10.1)

225%로 배율을 바꾸자 트레이 아이콘이 흐릿해졌다. 앱을 켜 둔 채 바꿨을 때만 생겼다.

### 로그

MSIX 설치본의 로그(`LocalState\log.txt`)에 단서가 남아 있었다.

| 시작 | `tray icon size` | 비고 |
| --- | --- | --- |
| 11:55 | 16px | 이 인스턴스가 실행 중에 배율이 225%로 바뀌었다 |
| 12:28 (재시작) | 36px | 재시작하면 크기가 맞다 |

`main.rs`가 시작할 때 `GetSystemMetrics(SM_CXSMICON)`을 한 번 읽어 `Tray`에 고정했다. 실행
중에는 크기가 바뀌지 않으므로 16px 비트맵을 셸이 36px 칸에 보간해 늘렸다.

### 왜 `GetSystemMetrics`를 다시 읽는 것으로는 안 되나

PerMonitorV2 프로세스의 `GetSystemMetrics`는 시스템 DPI 기준 값을 돌려주고, 시스템 DPI는
프로세스가 시작될 때 정해진다. 로그도 재시작해야 36px가 나왔다. 그래서
DPI를 명시하는 `GetSystemMetricsForDpi(SM_CXSMICON, dpi)`를 쓰고, dpi는 트레이를 실제로
그리는 주 작업 표시줄 창(`Shell_TrayWnd`)의 `GetDpiForWindow`에서 얻는다.

| 값 | 실행 중 배율 변경을 따라가나 |
| --- | --- |
| `GetSystemMetrics(SM_CXSMICON)` | 아니다. 위 로그처럼 재시작해야 36px가 나왔다 |
| `GetDpiForSystem()` | 아니다(시작 시점 값, 문서 기준. 따로 재지 않았다) |
| `GetDpiForWindow(Shell_TrayWnd)` | 따라간다. 아래 「실측」에서 크기가 16 → 36 → 16px로 바뀌었다 |

`tray-monitor --dpi --watch`가 세 값을 1초마다 비교해 출력한다. 100%에서의 출력:

```
system dpi  96  taskbar dpi   96  GetSystemMetrics(SM_CXSMICON) 16px  -> icon 16px (1x)
```

### 고친 것

| 항목 | 변경 |
| --- | --- |
| 크기 계산 | `win32::tray_icon_size` = `GetSystemMetricsForDpi(SM_CXSMICON, 작업 표시줄 DPI)`, 창이 없으면 `GetDpiForSystem` |
| 추적 | `App::tick`이 매 틱 다시 잰다. 타이머 창은 message-only라 `WM_DPICHANGED`/`WM_SETTINGCHANGE`를 받지 못해 폴링한다 |
| 반영 | `Tray::set_px`가 모든 슬롯을 `set_icon`으로 즉시 다시 그린다. 아이콘은 다시 만들지 않는다(규칙 1, 17) |
| 로그 | `tray icon size 16px -> 36px` |
| 렌더 | 2x 이상은 정수배 + 여백([PIXEL-FONT.md](PIXEL-FONT.md) §10) |

### 실측

수정한 빌드를 띄운 채 배율을 100% → 225% → 100%로 바꿨다(Windows 11, 2560x1440 단일 모니터).

```
13:04:54 tray icon size 16px
13:05:45 tray icon size 16px -> 36px
13:07:31 tray icon size 36px -> 16px
```

두 번 모두 재시작 없이 다음 틱에 다시 그려졌고, 225%에서 아이콘이 흐릿하지 않고 또렷했다(눈으로
판정). 경고와 오류는 없었다.

값 표본이 없는 지표(GPU가 없는 기기 등)는 `update`가 불리지 않는다. 다음 갱신을 기다리면
그 아이콘만 옛 크기로 남으므로 `set_px`는 저장된 값으로 바로 다시 그린다.


## 17. 디스크 사용량과 로드율 (v0.11.0)

관리자 권한 없이 디스크의 "활성 시간"을 읽을 수 있는지 확인했다. 기기: Windows 11, NVMe
`SAMSUNG MZVL22T0HBLB`(디스크 0, `C:`), USB 카드 리더(디스크 1, `F:`, 미디어 없음). `admin=False`.

### 접근 권한

| 호출 | 결과 |
| --- | --- |
| `CreateFileW("\\.\PhysicalDrive0", 0, READ\|WRITE, OPEN_EXISTING)` | 성공 |
| `IOCTL_DISK_PERFORMANCE`(0x70020) | 성공. `DISK_PERFORMANCE` 88바이트 |
| `CreateFileW("\\.\C:", 0, …)` + `IOCTL_VOLUME_GET_VOLUME_DISK_EXTENTS` | `extents=1 disk0` |
| `IOCTL_STORAGE_QUERY_PROPERTY`(StorageDeviceProperty) | 제품 `SAMSUNG MZVL22T0HBLB-00B00`, NVMe는 제조사 칸이 비어 있다 |
| `\\.\PhysicalDrive2`, `3` | `ERROR_FILE_NOT_FOUND`. 번호는 연속이 아니다 |

접근 권한 0은 메타데이터 질의만 허용한다. 읽기 권한을 달라고 하면 관리자가 필요하다.

### 공식

`IdleTime`과 `QueryTime`은 둘 다 100ns 누적값이다. 1초 동안:

| 디스크 | ΔIdleTime | ΔQueryTime | 활성 | PDH `% Idle Time` |
| --- | --- | --- | --- | --- |
| 0 (`C:`) | 8,837,258 | 10,257,306 | 13.84% | 84.84% → 15.16% |
| 1 (`F:`) | 10,044,992 | 10,048,698 | 0.04% | 100.01% → 0% |

`1 - ΔIdle/ΔQuery`는 작업 관리자 "활성 시간"과 같다. PDH와의 차이(1.3%p)는 두 측정의 구간이
조금 달라서다. 같은 호출이 `BytesRead`/`BytesWritten`도 주므로 툴팁의 읽기·쓰기 속도는 공짜다.

### 네트워크 인터페이스 목록

`GetIfTable2`는 이 기기에서 59개를 돌려줬다. WFP·QoS 필터 드라이버 인스턴스
(`Ethernet-WFP Native MAC Layer LightWeight Filter-0000` 등)와 WAN Miniport가 대부분이다.
`GetAdaptersAddresses`는 `Get-NetAdapter`와 같은 9개(+ 루프백)만 준다. 설정 창 목록은 이쪽을 쓰고,
집계는 그대로 `GetIfTable2`에서 **같은 별칭**(`Alias`)의 행 하나만 센다. 필터 인스턴스의 별칭은
`Ethernet-WFP …`처럼 접미사가 붙어 본체와 겹치지 않는다.

### 함정

- 핸들을 열어 두면 USB 드라이브의 "안전하게 제거"가 막힌다. 표본마다 열고 닫는다.
- 미디어가 없는 이동식 드라이브(`F:`)는 `GetDiskFreeSpaceExW`가 실패한다. 목록에서 뺀다.
