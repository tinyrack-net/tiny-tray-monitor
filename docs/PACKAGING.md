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
| 단일 인스턴스 | `Local\tray-monitor-single-instance` mutex가 패키지/비패키지 사이에서 공유됨 |
| 서명 없음 | 설치 불가: `0x800B0100`(unsigned), `0x800B0109`(신뢰되지 않은 자체 서명) |

manifest 최소 형태(검증됨):

```xml
<Package
  xmlns="http://schemas.microsoft.com/appx/manifest/foundation/windows10"
  xmlns:uap="http://schemas.microsoft.com/appx/manifest/uap/windows10"
  xmlns:uap5="http://schemas.microsoft.com/appx/manifest/uap/windows10/5"
  xmlns:rescap="http://schemas.microsoft.com/appx/manifest/foundation/windows10/restrictedcapabilities"
  IgnorableNamespaces="uap uap5 rescap">
  <Identity Name="Tinyrack.TrayMonitor" Publisher="CN=Tinyrack"
            Version="0.3.1.0" ProcessorArchitecture="x64" />
  <Properties>
    <DisplayName>tray-monitor</DisplayName>
    <PublisherDisplayName>Tinyrack</PublisherDisplayName>
    <Logo>Assets\StoreLogo.png</Logo>
  </Properties>
  <Resources><Resource Language="en-us" /></Resources>
  <Dependencies>
    <TargetDeviceFamily Name="Windows.Desktop"
      MinVersion="10.0.17763.0" MaxVersionTested="10.0.26100.0" />
  </Dependencies>
  <Capabilities><rescap:Capability Name="runFullTrust" /></Capabilities>
  <Applications>
    <Application Id="TrayMonitor" Executable="tray-monitor.exe"
                 EntryPoint="Windows.FullTrustApplication">
      <uap:VisualElements DisplayName="tray-monitor" Description="…"
        Square44x44Logo="Assets\Square44x44Logo.png"
        Square150x150Logo="Assets\Square150x150Logo.png" BackgroundColor="transparent">
        <uap:DefaultTile Wide310x150Logo="Assets\Wide310x150Logo.png" />
      </uap:VisualElements>
      <Extensions>
        <uap5:Extension Category="windows.startupTask"
                        Executable="tray-monitor.exe"
                        EntryPoint="Windows.FullTrustApplication">
          <uap5:StartupTask TaskId="TrayMonitorStartup" Enabled="false"
                            DisplayName="tray-monitor" />
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
   - 이미 존재하는 `%APPDATA%\tray-monitor` 안의 파일은 실제 경로에 쓴다(실측).
   - 즉 **설치 이력에 따라 설정/로그 위치가 달라진다.**
     `Open config file`이 빈 파일을 열 수 있다.
   - 대응: 패키지 모드에서는 `LocalState`를 명시적으로 쓴다.

3. **승격 감지 오탐**
   - 패키지 앱의 `NotifyIconSettings\<hash>\ExecutablePath`는
     `{6D809377-…}\WindowsApps\…` 형식이다(Command Palette/Codex 항목에서 확인).
   - `onboard.rs`의 정확한 문자열 비교는 항상 불일치 → 매번 숨김으로 판단.
   - 대응: known-folder GUID 접두사를 정규화해 비교.

4. **업데이트마다 경로가 바뀐다**
   - 설치 위치가 `WindowsApps\<Name>_<Version>_x64__<hash>\`라 버전이 바뀌면 exe 경로도 바뀐다.
   - 버전만 올려 재등록한 실험에서 기존 승격 키가 재사용되지 않고
     새 키가 `promoted=0`으로 생성됐다.
   - AGENTS.md #8("exe 경로를 바꾸지 않는다")과 충돌한다. MSIX 채널의 구조적 한계로 문서화한다.
   - 완화: 버전이 바뀌면 `should_onboard`가 다시 안내한다.

5. **패키지 앱은 `NotifyIconSettings`에 쓸 수 없다**
   - 쓰기가 가상 하이브로 가므로 앱이 스스로 승격을 복원할 수 없다(실측).
   - 완화: 온보딩 안내만 제공.

### 1.3 Installer는 문제 없다

| 항목 | 결과 |
| --- | --- |
| 컴파일 | Inno Setup 6.7.3, 1.3초 |
| 무인 설치 | `/VERYSILENT /CURRENTUSER /DIR=…` → exe, Uninstall 키, 시작 메뉴 바로가기 |
| 제거 | `unins000.exe /VERYSILENT` → 파일·Uninstall 키 제거 |
| AppMutex | `Local\tray-monitor-single-instance` 컴파일 확인 |
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
| 설치 범위 | 기본 **per-user**(`{localappdata}\Programs\tray-monitor`), `PrivilegesRequiredOverridesAllowed=commandline dialog` | 트레이 앱은 HKCU 설정과 per-user가 일치. 추후 관리자 권한이 필요해지면 설치 프로그램만 per-machine으로 전환하거나 드라이버를 별도 MSI로 분리 |
| 서명 | **이번 스코프 없음** (unsigned 빌드) | MSIX는 Store 제출 시 Microsoft가 서명. 일반 배포용 MSIX는 추후 인증서/Artifact Signing 필요 |
| MSIX 데이터 경로 | **패키지 인식 경로 분기** | Store 정책에 안전. `unvirtualizedResources` 불필요 |
| MSIX 자동 시작 | **StartupTask API** | 패키지에서 Run 키가 무동작 |
| 아이콘 | **간단한 임시 마크 생성 후 커밋** | 디자인은 나중에 교체. `tools/make_assets.py`로 재생성 |
| VC 런타임 | **정적 CRT** | MSIX/installer 모두 VC++ 재배포 요구 제거 |
| MSIX identity | 기본 `Tinyrack.TrayMonitor` / `CN=Tinyrack`, 환경 변수로 덮어쓰기 | 실제 Store identity는 Partner Center 등록 후 확정 |
| 버전 소스 | **빌드된 exe의 `ProductVersion`** | `build.rs`가 `Cargo.toml`에서 만들므로 아티팩트와 항상 일치. 태그와도 CI에서 대조 |

---

## 3. 산출물

태그 `vX.Y.Z` 릴리스에 다음을 첨부한다.

| 파일 | 설명 |
| --- | --- |
| `tray-monitor-X.Y.Z-windows-x64.zip` | 기존 포터블 |
| `tray-monitor-X.Y.Z-windows-x64-setup.exe` | Inno Setup installer |
| `tray-monitor-X.Y.Z-windows-x64.msix` | Store 제출용(현재 unsigned) |
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
pub fn config_dir() -> PathBuf;       // 패키지: LocalState, 그 외: %APPDATA%\tray-monitor
pub fn data_dir() -> PathBuf;         // 패키지: LocalState, 그 외: %LOCALAPPDATA%\tray-monitor
```

- `config.rs::config_dir()`, `logging.rs::data_dir()`가 이 모듈을 쓰도록 바꾼다.
  (`tray::open_config_file`, `settings::dialog::open_log_file`도 자동으로 따라온다.)
- 패키지 첫 실행 마이그레이션: `LocalState`에 설정이 없고
  `%APPDATA%\tray-monitor\config.toml`이 읽히면 복사한다(읽기는 가상화되지 않음 — 실측).
- 순수 경로 결정 함수(`resolve_dirs(packaged, pfn, appdata, localappdata)`)를 분리해 단위 테스트.
- `logging::init()`은 마이그레이션 뒤에 호출되도록 순서 확인.

`src/startup.rs`:

- `platform::packaged()`이면 `StartupTask::GetAsync("TrayMonitorStartup")` → 상태 조회/설정.
- 블로킹 대기 헬퍼: `IAsyncInfo::Status()`를 10ms 간격으로 폴링, 5초 타임아웃.
- 상태 매핑:
  - `Enabled`/`EnabledByPolicy` → 켜짐
  - `Disabled` → 끔
  - `DisabledByUser` → 끔 + `ms-settings:startupapps` 안내(앱이 켤 수 없음)
  - `DisabledByPolicy` → 끔 + 정책 안내
- 비패키지에서는 기존 HKCU Run 경로 유지.
- 설정 창과 트레이 메뉴가 같은 `startup::is_enabled()/set()`을 쓰므로 변경 지점은 한 곳이다.

`src/tray/onboard.rs`:

- `same_executable(registry_path, exe_path) -> bool` 순수 함수 추가.
  - `{GUID}\` 접두사를 제거하고, `WindowsApps\` 뒤 꼬리를 비교.
  - 비패키지 경로는 기존처럼 대소문자 무시 비교.
- 단위 테스트: 일반 경로, `{GUID}\WindowsApps\…`, 대소문자/구분자 차이.

`src/ipc.rs`:

- `WM_CLOSE` 처리: `runtime::request_exit()` 후 `LRESULT(0)`.
  installer가 업그레이드 전에 앱을 정상 종료시키는 데 쓴다.
- `src/runtime.rs`: `request_exit()` 추가(`App::request_exit`로 `exiting = true`).
- IPC 창은 `WS_POPUP` 일반 창이라 `FindWindowW`로 다른 프로세스가 찾을 수 있다(기존 테스트로 확인됨).

### 4.3 Installer (P3)

`packaging/windows/tray-monitor.iss`:

- `AppId` 고정 GUID, `DefaultDirName={autopf}\tray-monitor`,
  `PrivilegesRequired=lowest`, `PrivilegesRequiredOverridesAllowed=commandline dialog`.
- `AppMutex=Local\tray-monitor-single-instance`,
  `CloseApplications=yes`, `RestartApplications=no`.
- `[Code] PrepareToInstall`:
  - `FindWindow('tray-monitor-ipc', '')` → `PostMessage(WM_CLOSE)`.
  - 창이 사라질 때까지 최대 5초 대기. 그래도 있으면 종료를 요청하는 메시지.
- `[Registry]`: 언인스톨 시 `HKCU\...\Run\tray-monitor` 제거(`uninsdeletevalue`).
  설정 파일(`%APPDATA%\tray-monitor`)은 남긴다.
- `[Icons]`: 시작 메뉴 바로가기, 선택적 바탕 화면 아이콘.
- `[Run]`: `runasoriginaluser`, `skipifsilent`로 설치 후 실행.
- `SetupIconFile=resources\app.ico`, `UninstallDisplayIcon={app}\tray-monitor.exe`,
  `LicenseFile=LICENSE`, `MinVersion=10.0.17763`, 영문/한국어.
- 출력: `tray-monitor-<ver>-windows-x64-setup.exe`.

`packaging/windows/build-installer.ps1`:

- 빌드된 exe의 `VersionInfo.ProductVersion`에서 버전을 읽는다.
- `ISCC.exe` 탐색: `$env:ISCC` → `Program Files (x86)\Inno Setup 6` →
  `%LOCALAPPDATA%\Programs\Inno Setup 6`.
- `/DAppVersion`, `/DSourceExe`, `/DOutputDir`, `/DAppIcon` 전달.

### 4.4 MSIX (P4)

`packaging/windows/AppxManifest.xml`:

- 1.1의 검증된 manifest를 템플릿화: `@@VERSION@@`, `@@IDENTITY_NAME@@`, `@@PUBLISHER@@`,
  `@@PUBLISHER_DISPLAY_NAME@@`, `@@DISPLAY_NAME@@`, `@@ARCH@@`.
- 기본값 `Tinyrack.TrayMonitor`, `CN=Tinyrack`.
  환경 변수 `MSIX_IDENTITY_NAME`, `MSIX_PUBLISHER`, `MSIX_PUBLISHER_DISPLAY_NAME`로 덮어쓰기.
- 선택 사항: `uap5:AppExecutionAlias`(`desktop4:Subsystem="windows"`)로
  `tray-monitor --dump` 같은 진단 명령을 패키지 설치에서도 쓸 수 있게 한다(구현 시 검증).

`packaging/windows/build-msix.ps1`:

- exe의 `ProductVersion`을 4자리(`0.3.1` → `0.3.1.0`)로 변환. prerelease는 오류로 거부.
- staging에 exe + manifest + `Assets`를 모은다.
- `makeappx.exe` 탐색: `Windows Kits\10\bin\*\x64` 중 최신.
- `makeappx pack /d <staging> /p <out> /o`.
- 검증: `makeappx unpack` 왕복 + manifest/자산/exe 존재 확인.
- 서명 없음. 추후 인증서가 생기면 `signtool` 단계만 추가.
- 출력: `tray-monitor-<ver>-windows-x64.msix`.

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
    `%LOCALAPPDATA%\Packages\Tinyrack.TrayMonitor_<hash>\LocalState`,
    자동 시작은 Windows 설정의 Startup apps, 업데이트 후 트레이 승격이 초기화될 수 있음.
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
4. MSIX에서 설정 변경 → `LocalState`에 기록되는지, `Open config file`이 그 파일을 여는지.
5. MSIX 버전 업데이트 → 승격 초기화 + 온보딩 재안내(문서화된 동작).

---

## 6. 위험과 한계

| 위험 | 영향 | 대응 |
| --- | --- | --- |
| unsigned MSIX는 일반 사용자 설치 불가 | MSIX 자산은 Store 제출/개발자 모드 전용 | README에 명시. 서명은 후속 |
| Store identity 미확정 | `CN=Tinyrack`은 실제 제출 시 거부됨 | Partner Center 등록 후 환경 변수/시크릿으로 교체 |
| MSIX 업데이트 시 승격 초기화 | 사용자가 다시 트레이에 고정해야 함 | 온보딩 재안내, 문서화 |
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

- Partner Center에서 `Tinyrack.TrayMonitor` identity 예약, Publisher CN 확정.
- 코드 서명 인증서(Azure Artifact Signing 권장) 도입 → installer/MSIX 서명.
- MSIX `.msixbundle`(x64 + arm64), Store 제출 파이프라인.
- winget/scoop 매니페스트 자동 갱신.

---

## 9. 진행 결과 (v0.4.0)

| 단계 | 결과 |
| --- | --- |
| P1 | 자산 12개 생성·커밋, `app.rc` ICON, 정적 CRT, `[lints.rust]`. 테스트 **131개** 통과. exe 629,760 bytes, `VCRUNTIME140` 없음, 아이콘 32×32 확인 |
| P2 | `platform`/StartupTask/승격 정규화/WM_CLOSE. 패키지 등록 후 `LocalState`에 config·state·log가 생성되고 기존 `%APPDATA%` 설정이 복사되는 것을 확인. 업그레이드 시 설정 창/첫 실행 안내를 닫고 종료하는 것도 확인 |
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
   종료 요청(`WM_CLOSE`)은 프로세스의 `#32770` 창을 모두 닫는다. 설정 창이나 첫 실행
   안내가 모달 루프를 잡고 있으면 installer가 5초 뒤 포기하기 때문이다. 안내가 아직
   뜨기 전에 종료 요청이 오는 경우도 있어 `run_onboarding`이 `exiting`을 확인한다.
2. **`AppExecutionAlias`는 넣지 않았다.** Store 사용자에게 CLI를 노출할 필요가 아직 없다.
3. **아이콘은 임시 마크다.** `tools/make_assets.py`가 ICO와 MSIX 타일을 모두 만들고,
   디자인 교체 시 이 스크립트만 고치면 된다.
4. **로컬 Git Bash에는 `make`가 없다.** README/AGENTS에 스크립트 직접 실행 명령을 병기했다.
