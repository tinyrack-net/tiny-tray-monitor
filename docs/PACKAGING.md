# 배포 패키징 계획 — Inno Setup installer + MSIX

릴리스에 **Inno Setup installer**와 **Microsoft Store 제출용 MSIX**를 추가한다.
이번 스코프는 **빌드·검증까지**이며, Partner Center 등록과 실제 스토어 제출은 다루지 않는다.

- 기준 커밋: `5de46fd` (v0.3.1, 설정 GUI 포함)
- 관련 문서: [PLAN.md](PLAN.md), [SPIKE.md](SPIKE.md)
- 실측 환경: Windows 10.0.26300, Windows SDK 10.0.26100, Inno Setup 6.7.3

---

## 0. 현재 상태 (이미 되어 있는 것)

| 항목 | 상태 |
| --- | --- |
| 버전 단일 소스 | `Cargo.toml` → `build.rs`가 `version_defines.rc` 생성 |
| exe 리소스 | `resources/app.rc`: `VERSIONINFO`, 매니페스트(comctl32 v6, PerMonitorV2, asInvoker) |
| 리소스 컴파일 | `build.rs` + `embed-resource` |
| 설정 GUI | `resources/app.rc`의 `IDD_SETTINGS` + `src/settings/` |
| 두 번째 실행 | `src/ipc.rs`가 실행 중 인스턴스에 설정 창 요청 |
| 자동 시작 | `src/startup.rs` (HKCU Run) — 메뉴와 설정 창이 같은 경로 사용 |
| 패키징 | **없음** (exe 아이콘 리소스도 없음, VC 런타임 동적 링크) |

이번 작업은 이 위에 installer/MSIX를 얹는 것이다.

---

## 1. 실측 결과 (검증 완료)

### 1.1 MSIX는 빌드·설치·실행된다

v0.3.1 exe로 확인했다.

| 항목 | 결과 |
| --- | --- |
| 패키징 | `makeappx pack` 성공 |
| 실행 | `Add-AppxPackage -Register`(개발자 모드) 후 트레이 4개 정상 갱신 |
| 설정 창 | 패키지 앱에서 정상 표시 |
| IPC | 두 번째 실행이 첫 인스턴스에 설정 창을 열게 함(`설정 창을 열었다` 로그) |
| 자동 시작 | `uap5:Extension windows.startupTask` + `StartupTask::RequestEnableAsync` → `Enabled(2)` |
| 단일 인스턴스 | `Local\TinyTrayMonitor.SingleInstance` mutex가 패키지/비패키지 사이에서 공유됨 |
| 서명 없음 | 설치 불가: `0x800B0100`(unsigned), `0x800B0109`(신뢰되지 않은 자체 서명) |

manifest 최소 형태(검증됨):

```xml
<Package
  xmlns="http://schemas.microsoft.com/appx/manifest/foundation/windows10"
  xmlns:uap="http://schemas.microsoft.com/appx/manifest/uap/windows10"
  xmlns:uap5="http://schemas.microsoft.com/appx/manifest/uap/windows10/5"
  xmlns:rescap="http://schemas.microsoft.com/appx/manifest/foundation/windows10/restrictedcapabilities"
  IgnorableNamespaces="uap uap5 rescap">
  <Identity Name="tinyrack.TinyTrayManager" Publisher="CN=42FA8D93-3683-4A00-A40C-5ED8A02E8B3A"
            Version="0.5.0.0" ProcessorArchitecture="x64" />
  <Properties>
    <DisplayName>TinyTrayManager</DisplayName>
    <PublisherDisplayName>tinyrack</PublisherDisplayName>
    <Logo>Assets\StoreLogo.png</Logo>
  </Properties>
  <Resources><Resource Language="en-us" /></Resources>
  <Dependencies>
    <TargetDeviceFamily Name="Windows.Desktop"
      MinVersion="10.0.17763.0" MaxVersionTested="10.0.26100.0" />
  </Dependencies>
  <Capabilities><rescap:Capability Name="runFullTrust" /></Capabilities>
  <Applications>
    <Application Id="TinyTrayMonitor" Executable="tiny-tray-monitor.exe"
                 EntryPoint="Windows.FullTrustApplication">
      <uap:VisualElements DisplayName="TinyTrayMonitor" Description="…"
        Square44x44Logo="Assets\Square44x44Logo.png"
        Square150x150Logo="Assets\Square150x150Logo.png" BackgroundColor="transparent">
        <uap:DefaultTile Wide310x150Logo="Assets\Wide310x150Logo.png" />
      </uap:VisualElements>
      <Extensions>
        <uap5:Extension Category="windows.startupTask"
                        Executable="tiny-tray-monitor.exe"
                        EntryPoint="Windows.FullTrustApplication">
          <uap5:StartupTask TaskId="TinyTrayMonitorStartup" Enabled="true"
                            DisplayName="TinyTrayMonitor" />
        </uap5:Extension>
      </Extensions>
    </Application>
  </Applications>
</Package>
```

> `desktop6:*WriteVirtualization`로 가상화를 끄려면 `unvirtualizedResources`
> restricted capability가 필요하고 Store 승인 대상이다. 사용하지 않는다.

### 1.2 패키지 앱에서 그대로 두면 깨지는 동작

1. **레지스트리 쓰기 가상화**
   - 패키지 앱이 `HKCU\...\Run`에 쓴 값이 실제 하이브에 나타나지 않는다(실측).
   - `startup.rs`의 `Run at sign-in`이 패키지 빌드에서 **무동작**이 된다.
   - 대응: 패키지 모드에서 `StartupTask` API 사용.

2. **파일 쓰기 가상화**
   - `%APPDATA%`에 **새 폴더**를 만들면
     `%LOCALAPPDATA%\Packages\<PFN>\LocalCache\Roaming`으로 리다이렉트된다.
   - 이미 존재하는 `%APPDATA%\TinyTrayMonitor` 안의 파일은 실제 경로에 쓴다(실측).
   - 즉 **설치 이력에 따라 설정/로그 위치가 달라진다.**
     설정 창이 바꾼 값이 다른 파일에 저장될 수 있다.
   - 대응: 패키지 모드에서는 `LocalState`를 명시적으로 쓴다.

3. **업데이트마다 경로가 바뀐다**
   - 설치 위치가 `WindowsApps\<Name>_<Version>_x64__<hash>\`라 버전이 바뀌면 exe 경로도 바뀐다.
   - 대응: 지표별 고정 GUID(`tray::TRAY_GUIDS`)로 등록한다. Windows가 승격 설정을 GUID에
     묶으므로 경로가 바뀌어도 유지된다. 실측: 0.4.0.0 → 0.4.1.0(다른 폴더)로 등록을 바꿔도
     같은 4개 키가 새 경로로 옮겨지고 `IsPromoted=1`이 유지됐다(Windows 11 build 26300).
   - 셸이 GUID 등록을 실제로 거부할 때(`NIM_ADD` 또는 `NIF_GUID` 툴팁 갱신 실패)만 GUID
     없이 다시 등록한다. `NotifyIconSettings` 키가 없다는 것은 거부 신호가 아니다
     ([SPIKE.md](SPIKE.md) §15).

4. **패키지 앱은 `NotifyIconSettings`에 쓸 수 없다**
   - 쓰기가 가상 하이브로 가므로 앱이 스스로 승격을 복원할 수 없다(실측).
   - 앱은 이 키를 읽지도 않는다. 무엇을 고정할지는 사용자가 정한다([SPIKE.md](SPIKE.md) §15).

### 1.3 Installer는 문제 없다

| 항목 | 결과 |
| --- | --- |
| 컴파일 | Inno Setup 6.7.3, 1.3초 |
| 무인 설치 | `/VERYSILENT /CURRENTUSER /DIR=…` → exe, Uninstall 키, 시작 메뉴 바로가기 |
| 제거 | `unins000.exe /VERYSILENT` → 파일·Uninstall 키 제거 |
| AppMutex | `Local\TinyTrayMonitor.SingleInstance` 컴파일 확인 |
| Run 값 정리 | `[Registry] … Flags: uninsdeletevalue` 컴파일 확인 |
| CI 러너 | `windows-2025`에 InnoSetup 6.7.1 사전 설치(`C:\Program Files (x86)\Inno Setup 6`) |
| 로컬 설치 위치 | winget 설치 시 `%LOCALAPPDATA%\Programs\Inno Setup 6` → 두 경로 모두 탐색 필요 |

### 1.4 빌드 메타데이터

- 현재 exe: `VERSIONINFO` 있음(`0.3.1`), **아이콘 리소스 없음**(`app.rc`에 `ICON` 없음).
- Rust 기본 빌드: `VCRUNTIME140.dll` 의존.
  `target-feature=+crt-static` + `[lints.rust] warnings = "deny"` + `.cargo/config.toml`로
  의존성이 사라지고 테스트 131개가 모두 통과한다(실측, +103KB).
- `resources/app.rc`에 `1 ICON "resources/app.ico"` 한 줄이면 아이콘이 링크된다(실측).
- `embed-resource`는 `rc.exe`의 include 경로를 스스로 채운다(SPIKE §10-1).
- `windows` crate: `GetCurrentPackageFullName`(`Win32_Storage_Packaging_Appx`),
  `StartupTask`(`ApplicationModel`) 사용 가능.
  `windows-future`의 `IAsyncOperation`은 공개 블로킹 대기가 없어 `IAsyncInfo::Status()` 폴링 필요(검증됨).
- CI `windows-2025`에는 SDK 10.0.26100이 있지만 `makeappx.exe`/`signtool.exe`는 PATH에 없다.

---

## 2. 결정

| 항목 | 결정 | 근거 |
| --- | --- | --- |
| Installer | **Inno Setup 6** | 조직 표준(tinest)과 동일, 러너 사전 설치, 사용자 단위 설치로 UAC 불필요, 경로 고정 |
| 설치 범위 | 기본 **per-user**(`{localappdata}\Programs\TinyTrayMonitor`), `PrivilegesRequiredOverridesAllowed=commandline dialog` | 트레이 앱은 HKCU 설정과 per-user가 일치. 추후 관리자 권한이 필요해지면 설치 프로그램만 per-machine으로 전환하거나 드라이버를 별도 MSI로 분리 |
| 서명 | **이번 스코프 없음** (unsigned 빌드) | MSIX는 Store 제출 시 Microsoft가 서명. 일반 배포용 MSIX는 추후 인증서/Artifact Signing 필요 |
| MSIX 데이터 경로 | **패키지 인식 경로 분기** | Store 정책에 안전. `unvirtualizedResources` 불필요 |
| MSIX 자동 시작 | **StartupTask API** | 패키지에서 Run 키가 무동작 |
| 아이콘 | **간단한 임시 마크 생성 후 커밋** | 디자인은 나중에 교체. `tools/make_assets.py`로 재생성 |
| VC 런타임 | **정적 CRT** | MSIX/installer 모두 VC++ 재배포 요구 제거 |
| MSIX identity | 예약된 `tinyrack.TinyTrayManager` / `CN=42FA8D93-…`, 환경 변수로 덮어쓰기 | Store ID `9MT22CHX25FD`. 이름·Publisher는 한 번 정하면 바꾸지 않는다 |
| 버전 소스 | **빌드된 exe의 `ProductVersion`** | `build.rs`가 `Cargo.toml`에서 만들므로 아티팩트와 항상 일치. 태그와도 CI에서 대조 |

---

## 3. 산출물

태그 `vX.Y.Z` 릴리스에 다음을 첨부한다.

| 파일 | 설명 |
| --- | --- |
| `tiny-tray-monitor-X.Y.Z-windows-x64.zip` | 기존 포터블 |
| `tiny-tray-monitor-X.Y.Z-windows-x64-setup.exe` | Inno Setup installer |
| `tiny-tray-monitor-X.Y.Z-windows-x64.msix` | Store 제출용(현재 unsigned) |
| `SHA256SUMS` | 위 세 파일(LF 개행 유지) |

---

## 4. 구현 계획

### 4.1 빌드 메타데이터 (P1)

- `tools/make_assets.py` (표준 라이브러리만):
  - `resources/app.ico` (16/20/24/32/40/48/64/128/256)
  - `resources/msix/{StoreLogo,Square44x44Logo,Square150x150Logo,Wide310x150Logo}.png`
  - `Square44x44Logo.targetsize-{16,24,32,48,256}.png`,
    `Square44x44Logo.altform-unplated.png`
  - 임시 마크: 어두운 라운드 사각형 + 길이가 다른 3개 막대(지표 모티프).
  - 생성물은 커밋한다(빌드 순서 문제 방지). `make assets`로 재생성.
- `resources/app.rc`: `1 ICON "resources/app.ico"` 추가.
- `.cargo/config.toml`:
  ```toml
  [target.'cfg(all(windows, target_env = "msvc"))']
  rustflags = ["-C", "target-feature=+crt-static"]
  ```
- `Cargo.toml`: `[lints.rust] warnings = "deny"` 추가.
  CI의 `RUSTFLAGS: -D warnings`는 `.cargo/config.toml`의 rustflags를 **덮어쓰므로 제거**한다.
- 플레이키 테스트 수정: `metrics::cpu::tests::meter_needs_two_samples`는
  `new()`와 `sample()` 사이에 틱이 지나면 실패한다(부하 중 1회 재현).
  테스트에서 `CpuMeter { prev: None }`을 직접 만들어 결정적으로 바꾼다.

### 4.2 패키지 인식 런타임 (P2)

새 모듈 `src/platform.rs`:

```rust
pub fn packaged() -> bool;            // GetCurrentPackageFullName != APPMODEL_ERROR_NO_PACKAGE
pub fn package_family_name() -> Option<String>;
pub fn config_dir() -> PathBuf;       // 패키지: LocalState, 그 외: %APPDATA%\TinyTrayMonitor
pub fn data_dir() -> PathBuf;         // 패키지: LocalState, 그 외: %LOCALAPPDATA%\TinyTrayMonitor
```

- `config.rs::config_dir()`, `logging.rs::data_dir()`가 이 모듈을 쓰도록 바꾼다.
- 패키지 첫 실행 마이그레이션: `LocalState`에 설정이 없고
  `%APPDATA%\TinyTrayMonitor\config.toml`이 읽히면 복사한다(읽기는 가상화되지 않음 — 실측).
- 순수 경로 결정 함수(`resolve_dirs(packaged, pfn, appdata, localappdata)`)를 분리해 단위 테스트.
- `logging::init()`은 마이그레이션 뒤에 호출되도록 순서 확인.

`src/startup.rs`:

- `platform::packaged()`이면 `StartupTask::GetAsync("TinyTrayMonitorStartup")` → 상태 조회/설정.
- 블로킹 대기 헬퍼: `IAsyncInfo::Status()`를 10ms 간격으로 폴링, 5초 타임아웃.
- 상태 매핑:
  - `Enabled`/`EnabledByPolicy` → 켜짐
  - `Disabled` → 끔
  - `DisabledByUser` → 끔 + `ms-settings:startupapps` 안내(앱이 켤 수 없음)
  - `DisabledByPolicy` → 끔 + 정책 안내
- 비패키지에서는 기존 HKCU Run 경로 유지.
- 설정 창과 트레이 메뉴가 같은 `startup::is_enabled()/set()`을 쓰므로 변경 지점은 한 곳이다.

`src/ipc.rs`:

- `WM_CLOSE` 처리: `runtime::request_exit()` 후 `LRESULT(0)`.
  installer가 업그레이드 전에 앱을 정상 종료시키는 데 쓴다.
- `src/runtime.rs`: `request_exit()` 추가(`App::request_exit`로 `exiting = true`).
- IPC 창은 `WS_POPUP` 일반 창이라 `FindWindowW`로 다른 프로세스가 찾을 수 있다(기존 테스트로 확인됨).

### 4.3 Installer (P3)

`packaging/windows/tiny-tray-monitor.iss`:

- `AppId` 고정 GUID, `DefaultDirName={autopf}\TinyTrayMonitor`,
  `PrivilegesRequired=lowest`, `PrivilegesRequiredOverridesAllowed=commandline dialog`.
- `AppMutex=Local\TinyTrayMonitor.SingleInstance`,
  `CloseApplications=yes`, `RestartApplications=no`.
- `[Code] PrepareToInstall`:
  - `FindWindow('TinyTrayMonitor.Ipc', '')` → `PostMessage(WM_CLOSE)`.
  - 창이 사라질 때까지 최대 5초 대기. 그래도 있으면 종료를 요청하는 메시지.
- `[Registry]`: 언인스톨 시 `HKCU\...\Run\TinyTrayMonitor` 제거(`uninsdeletevalue`).
  설정 파일(`%APPDATA%\TinyTrayMonitor`)은 남긴다.
- `[Icons]`: 시작 메뉴 바로가기, 선택적 바탕 화면 아이콘.
- `[Run]`: `runasoriginaluser`, `skipifsilent`로 설치 후 실행.
- `SetupIconFile=resources\app.ico`, `UninstallDisplayIcon={app}\tiny-tray-monitor.exe`,
  `LicenseFile=LICENSE`, `MinVersion=10.0.17763`, 영문/한국어.
- 출력: `tiny-tray-monitor-<ver>-windows-x64-setup.exe`.

`packaging/windows/build-installer.ps1`:

- 빌드된 exe의 `VersionInfo.ProductVersion`에서 버전을 읽는다.
- `ISCC.exe` 탐색: `$env:ISCC` → `Program Files (x86)\Inno Setup 6` →
  `%LOCALAPPDATA%\Programs\Inno Setup 6`.
- `/DAppVersion`, `/DSourceExe`, `/DOutputDir`, `/DAppIcon` 전달.

### 4.4 MSIX (P4)

`packaging/windows/AppxManifest.xml`:

- 1.1의 검증된 manifest를 템플릿화: `@@VERSION@@`, `@@IDENTITY_NAME@@`, `@@PUBLISHER@@`,
  `@@PUBLISHER_DISPLAY_NAME@@`, `@@DISPLAY_NAME@@`, `@@ARCH@@`.
- 기본값 `tinyrack.TinyTrayManager`, `CN=42FA8D93-3683-4A00-A40C-5ED8A02E8B3A`.
  환경 변수 `MSIX_IDENTITY_NAME`, `MSIX_PUBLISHER`, `MSIX_PUBLISHER_DISPLAY_NAME`로 덮어쓰기.
- 선택 사항: `uap5:AppExecutionAlias`(`desktop4:Subsystem="windows"`)로
  `tiny-tray-monitor --dump` 같은 진단 명령을 패키지 설치에서도 쓸 수 있게 한다(구현 시 검증).

`packaging/windows/build-msix.ps1`:

- exe의 `ProductVersion`을 4자리(`0.3.1` → `0.3.1.0`)로 변환. prerelease는 오류로 거부.
- staging에 exe + manifest + `Assets`를 모은다.
- `makeappx.exe` 탐색: `Windows Kits\10\bin\*\x64` 중 최신.
- `makeappx pack /d <staging> /p <out> /o`.
- 검증: `makeappx unpack` 왕복 + manifest/자산/exe 존재 확인.
- 서명 없음. 추후 인증서가 생기면 `signtool` 단계만 추가.
- 출력: `tiny-tray-monitor-<ver>-windows-x64.msix`.

`packaging/windows/Assets/`:

- `tools/make_assets.py`가 만든 PNG를 커밋한다.

### 4.5 CI (P5)

`.github/workflows/pipeline.yml`:

- `env.RUSTFLAGS: -D warnings` 제거(`[lints.rust]`로 대체).
- `package` 잡 추가 (`windows-2025`, `needs: build`, PR/merge queue/태그 모두 실행):
  - `build`의 exe 아티팩트 다운로드.
  - `build-installer.ps1`, `build-msix.ps1` 실행.
  - installer 스모크: `/VERYSILENT /CURRENTUSER /DIR=$RUNNER_TEMP/...` →
    exe 존재 확인 → `--help` 실행 → `unins000.exe /VERYSILENT` → 제거 확인.
  - MSIX 검증: `makeappx pack` + `unpack`, manifest XML 파싱, 자산 존재 확인.
  - `dist/*` 업로드.
- `quality-gate.needs`에 `package` 추가.
- `release`:
  - `needs: package`, 아티팩트 다운로드.
  - 태그(`vX.Y.Z`)와 exe `ProductVersion` 일치 확인.
  - 포터블 zip + setup.exe + msix + `SHA256SUMS`(LF) 작성.
  - `gh release create`.

### 4.6 문서 (P6)

- `README.md`:
  - 설치 방법 3가지(installer 권장, 포터블, MSIX는 Store 제출용/unsigned 주의).
  - MSIX 사용자: 설정/로그는
    `%LOCALAPPDATA%\Packages\tinyrack.TinyTrayManager_<hash>\LocalState`,
    자동 시작은 Windows 설정의 Startup apps, 업데이트 후에도 고정 GUID로 승격이 유지됨.
  - MSIX에서 Start 메뉴로 실행하면 창 없이 트레이만 시작되고,
    다시 실행하거나 트레이 메뉴로 설정을 연다는 점.
- `AGENTS.md`: 구조(`packaging/`), 릴리스 산출물, `make package`,
  MSIX 제약(경로 불변성, 가상화) 추가.
- `docs/PLAN.md`: M7/§12.9의 "설치 프로그램" 항목을 이 문서로 연결.
- `docs/PACKAGING.md`: 이 문서.

---

## 5. 검증 계획

| 대상 | 방법 |
| --- | --- |
| 경로 결정, 버전 변환, 승격 경로 정규화, StartupTask 상태 매핑 | 순수 함수 단위 테스트 |
| exe 메타데이터 | 빌드 후 VERSIONINFO/아이콘 확인 |
| installer | 무인 설치/제거, 앱 실행 중 업그레이드(WM_CLOSE 유도), 시작 메뉴 바로가기 |
| MSIX | `makeappx pack/unpack`, 개발자 모드 `-Register`로 트레이 + 설정 창 + 자동 시작 토글 |
| CI | PR에서 `package` 통과, 태그에서 자산 3종 + SHA256SUMS |
| 회귀 | `cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test` (131개 기준) |

수동 확인 체크리스트:

1. installer 설치 → 트레이 4개, 승격 유지, 설정 창 열림.
2. 앱 실행 중 installer 업그레이드 → 자동 종료 후 설치, 재실행.
3. MSIX `-Register` → 트레이, 설정 창, `Run at sign-in` 토긘이 StartupTask를 켜는지.
4. MSIX에서 설정 변경 → `LocalState\config.toml`에 기록되는지.
5. MSIX 버전 업데이트 → 같은 GUID 키가 새 경로로 옮겨지고 승격 유지, 온보딩 없음.
6. 새 설치 직후 `HKCU\...\Run`에 `TinyTrayMonitor` 값이 있는지, 설정에서 끄면 사라지는지,
   업그레이드해도 다시 생기지 않는지(§11).

---

## 6. 위험과 한계

| 위험 | 영향 | 대응 |
| --- | --- | --- |
| unsigned MSIX는 일반 사용자 설치 불가 | MSIX 자산은 Store 제출/개발자 모드 전용 | README에 명시. 서명은 후속 |
| Store identity | 예약 완료: `tinyrack.TinyTrayManager`, `CN=42FA8D93-…` | 이름·Publisher는 제출 후 바꾸지 않는다. 변경하려면 새 제품을 등록해야 한다 |
| 셸이 고정 GUID 등록을 거부(`NIM_ADD`/툴팁 실패) | 승격이 경로에 묶임(아이콘은 보임) | GUID 없이 재등록, 온보딩 재안내 |
| MSIX에서 Start 메뉴 실행 시 창이 없다 | 사용자가 앱이 실행됐는지 모를 수 있음 | 트레이/두 번째 실행/`--settings` 안내, 온보딩 |
| MSIX와 installer 동시 설치 | 자동 시작 항목 중복, 하나만 실행됨(mutex) | 문서화. 필요 시 installer가 MSIX 설치 감지 후 안내 |
| per-user ↔ per-machine 전환 시 경로 변경 | 승격 초기화 | 문서화 |
| `makeappx`/`ISCC` PATH 미등록 | 스크립트 실패 | 최신 SDK/표준 설치 경로 탐색 + 명확한 오류 |
| `make`가 로컬 Git Bash에 없음 | `make package` 실행 불가 | 스크립트를 직접 실행하는 명령을 README/AGENTS에 병기 |
| 정적 CRT | +102KB, CRT 보안 패치 재빌드 필요 | 릴리스마다 최신 toolchain 사용 |

---

## 7. 마일스톤

| 단계 | 내용 | 완료 조건 |
| --- | --- | --- |
| P1 | 아이콘/자산 생성, `app.rc` ICON, 정적 CRT, lints, 플레이키 테스트 수정 | `cargo test` 131개 통과, exe에 아이콘·버전 표시, VC 런타임 의존 없음 |
| P2 | `platform` 모듈, StartupTask, 승격 정규화, WM_CLOSE/`request_exit` | 단위 테스트 + 비패키지 회귀 통과 |
| P3 | installer 스크립트/빌드 스크립트 | 무인 설치·제거·업그레이드 확인 |
| P4 | MSIX manifest/자산/빌드 스크립트 | `makeappx pack/unpack` + `-Register` 실행 확인 |
| P5 | CI `package`/`release`/`quality-gate` 수정 | PR 통과, 태그 릴리스에 3종 자산 + SHA256SUMS |
| P6 | README/AGENTS/PLAN 정리 | 문서 링크·경로·제약 반영 |

---

## 8. 후속 (이번 스코프 아님)

- Partner Center identity 예약 완료: `tinyrack.TinyTrayManager` / `CN=42FA8D93-…` (Store ID `9MT22CHX25FD`).
- 코드 서명 인증서(Azure Artifact Signing 권장) 도입 → installer/MSIX 서명.
- MSIX `.msixbundle`(x64 + arm64), Store 제출 파이프라인([RELEASE-AUTOMATION.md](RELEASE-AUTOMATION.md)).
- winget/scoop 매니페스트 자동 갱신.

---

## 9. 진행 결과 (v0.4.0)

| 단계 | 결과 |
| --- | --- |
| P1 | 자산 12개 생성·커밋, `app.rc` ICON, 정적 CRT, `[lints.rust]`. 테스트 **131개** 통과. exe 629,760 bytes(P2 기능 포함 최종 642,048), `VCRUNTIME140` 없음, 아이콘 32×32 확인 |
| P2 | `platform`/StartupTask/WM_CLOSE. 패키지 등록 후 `LocalState`에 config·log가 생성되고 기존 `%APPDATA%` 설정이 복사되는 것을 확인. 업그레이드 시 설정 창을 닫고 종료하는 것도 확인 |
| P3 | 무인 설치 → `--help` → 실행 중 업그레이드(WM_CLOSE로 종료) → 제거까지 확인. 제거 시 HKCU Run 값도 삭제됨 |
| P4 | `makeappx pack/unpack` 왕복, 개발자 모드 등록, 트레이·설정 창·자동 시작 토글을 재시작 후까지 확인(`autostart=true/false`) |
| P5 | 워크플로 YAML 파싱, 릴리스 스크립트를 로컬에서 모의 실행해 3종 자산 + `SHA256SUMS` 생성 확인. 실제 CI는 PR에서 확인 |
| P6 | README/AGENTS/PLAN/Makefile 갱신 |

### 실행 중 바뀐 결정

1. **`StartupTask`는 `WM_COMMAND` 안에서 부르면 안 된다.** `RequestEnableAsync`가
   `RPC_E_CANTCALLOUT_ININPUTSYNCCALL`로 실패한다(실측). 처음에는 MTA 워커 스레드로
   우회했지만, 설정 창의 `WM_INITDIALOG` 안에서는 UI 스레드가 결과를 기다리는 동안
   대화상자가 보이지 않고 멈췄다. 최종적으로 자동 시작 토글을 `WM_APP_AUTOSTART`
   메시지로 미루고, UI 스레드에서 input-synchronous 호출 밖에 실행하도록 했다.
   종료 요청(`WM_CLOSE`)은 프로세스의 `#32770` 창을 모두 닫는다. 설정 창이 모달 루프를
   잡고 있으면 installer가 5초 뒤 포기하기 때문이다.
2. **`AppExecutionAlias`는 넣지 않았다.** Store 사용자에게 CLI를 노출할 필요가 아직 없다.
3. **아이콘은 임시 마크다.** `tools/make_assets.py`가 ICO와 MSIX 타일을 모두 만들고,
   디자인 교체 시 이 스크립트만 고치면 된다.
4. **로컬 Git Bash에는 `make`가 없다.** README/AGENTS에 스크립트 직접 실행 명령을 병기했다.

---

## 10. 고정 GUID (P7)

MSIX 업데이트마다 exe 경로가 바뀌어 트레이 승격이 초기화되던 문제를 지표별 고정 GUID로 해결했다.

- `src/tray/mod.rs`의 `TRAY_GUIDS`가 `Metric::ALL` 순서와 1:1로 대응한다. 값과 순서는
  한 번 정해지면 바꾸지 않는다. 바꾸면 사용자의 승격 설정이 다른 아이콘으로 옮겨간다.
- `win32::set_guid_tooltip`이 `NIF_TIP | NIF_GUID`로 `NIM_MODIFY`를 직접 보낸다.
  `tray-icon` 0.26.1의 `set_tooltip`은 `NIF_GUID`를 빠뜨려 GUID 아이콘에서 항상 실패한다.
- `Tray::build`의 수락 판정은 `NIM_ADD` 성공과 `set_guid_tooltip`의 성공뿐이다. **키가
  없다고 폴백하지 않는다**: 셸은 이미 아는 정체성에는 키를 (다시) 쓰지 않으므로, 키 생성
  여부를 거부 신호로 읽으면 두 번째 실행부터 항상 폴백하고 승격이 경로에 묶인다
  ([SPIKE.md](SPIKE.md) §15).

실측(Windows 11 build 26300, 74개 항목):

| 실험 | 결과 |
| --- | --- |
| unpackaged, unsigned, 고정 GUID | 4개 키에 `IconGuid` 기록, `IsPromoted` 유지 |
| unpackaged, 경로 A → B 이동 | 같은 키가 B 경로로 이동, `IsPromoted=1` 유지 |
| MSIX 0.4.0.0 → 0.4.1.0(다른 폴더) | 같은 키가 새 경로로 이동, `IsPromoted=1` 유지 |
| 이 세션이 이미 본 GUID로 다시 등록 | 키가 새로 생기지 않고 `NIM_ADD`/`NIM_MODIFY`는 성공 — 모순이 아니라 정상(§15) |

v0.7.0에서 6개 지표로 확인한 결과(Explorer 재시작 → 6개 키에 `IconGuid`, `promote.py`로 4개
고정 → exe를 다른 폴더로 복사해 실행해도 같은 키가 새 경로로 이동하며 `IsPromoted=1` 유지)는
[SPIKE.md](SPIKE.md) §15에 있다.

코드 서명은 필요하지 않았다. 서명이 있으면 `Publisher`가 기록될 뿐, 승격 유지 자체는
GUID만으로 성립했다.

> 주의: 실행 중인 앱을 `taskkill /F`로 죽이면 죽은 창의 정체성이 Explorer 세션에 남아,
> 같은 GUID·경로로 다시 등록해도 키가 기록되지 않는다. 그때는 Explorer를 재시작해야 한다.
> 정상 종료 경로(`Exit`, IPC 창의 `WM_CLOSE`)를 쓴다.

---

## 11. 새 설치의 자동 시작 기본값 (P8)

새로 설치하면 `Run at sign-in`이 **켜진 상태**로 시작한다. 업그레이드는 사용자가 앱 설정에서
바꾼 값을 건드리지 않는다.

| 배포 | 켜는 지점 | 업그레이드에서 다시 켜지지 않는 이유 |
| --- | --- | --- |
| Inno installer | `[Tasks]`의 `autostart`(`checkedonce`) + `[Registry]`의 HKCU Run 값 | `checkedonce`는 이전 버전이 설치되어 있으면 작업을 기본 해제한다(설치 문서: "unchecked initially when Setup finds a previous version"). 선택되지 않은 `[Registry]` 항목은 처리되지 않으므로 기존 값이 그대로 남는다 |
| MSIX | `AppxManifest.xml`의 `uap5:StartupTask Enabled="true"` | `Enabled`는 "**first starts** enabled or disabled"(문서) — 설치 시점의 초기 상태만 정한다. 사용자가 Task Manager나 Windows 설정에서 끄면 유지된다 |

- 제거는 두 경로 모두 Run 값을 지운다(`[Registry] Flags: uninsdeletevalue` + `[Code]`의
  `RegDeleteValue`). 앱 설정에서 직접 켠 값도 남지 않는다.
- 설치 프로그램의 마지막 페이지 "Launch TinyTrayMonitor" 체크박스(기본 체크)는 그대로 둔다.
  무인 설치에서는 실행하지 않는다(`skipifsilent`) — CI의 무인 설치/제거 스모크가 앱이 떠
  있는 상태와 부딪히지 않게 한다.
- 값 형식은 앱과 같아야 한다: `"C:\...\tiny-tray-monitor.exe"`(따옴표 포함). `startup.rs`의
  `is_enabled_named`가 문자열을 그대로 비교한다.
- 포터블 zip은 설치 단계가 없어 예전처럼 꺼진 상태로 시작한다.

## 12. 이름 (v0.11.0 → v0.12.0)

v0.11.0에서 TinyTrayManager / `tray-monitor.exe`를 TinyTrayMonitor / `tiny-tray-monitor.exe`로 바꾸며 옛 이름을
옮기는 코드를 넣었다가, v0.12.0에서 하위 호환을 버리고 전부 걷어 냈다. 옛 버전은 제거한 뒤 새로 설치한다.

| 대상 | 이름 | 관례 |
| --- | --- | --- |
| 표시 이름, `FileDescription`, `ProductName`, MSIX `DisplayName` | `TinyTrayMonitor` | 제품 이름 |
| 설치 폴더 | `{autopf}\TinyTrayMonitor` (`UsePreviousAppDir=no`) | `Program Files`·`Programs` 아래는 제품 이름 |
| 설정·로그 폴더 | `%APPDATA%`·`%LOCALAPPDATA%\TinyTrayMonitor` | 제품 이름 |
| 시작 메뉴·바탕 화면 바로 가기, `Run` 값 | `TinyTrayMonitor` | 제품 이름 |
| MSIX `Application Id` / `TaskId` | `TinyTrayMonitor` / `TinyTrayMonitorStartup` | 영숫자만 허용 |
| mutex, 창 클래스 | `Local\TinyTrayMonitor.SingleInstance`, `TinyTrayMonitor.Ipc`, `TinyTrayMonitor.Timer` | `제품.용도` |
| 실행 파일, crate, 릴리스 파일 | `tiny-tray-monitor` | 명령줄 도구 관례(소문자·하이픈) |
| **예외** MSIX `Identity Name` | `tinyrack.TinyTrayManager` | Partner Center가 제품마다 정한다. 바꾸면 새 제품 |
| installer `AppId` | `{3E1D7B2A-…}` | GUID라 이름과 무관 |

트레이 아이콘 GUID(`tray::TRAY_GUIDS`)도 그대로다. exe 이름을 바꿔도 8개 고정이 모두 유지되는 것을 쟀다
(`docs/SPIKE.md` §18).
