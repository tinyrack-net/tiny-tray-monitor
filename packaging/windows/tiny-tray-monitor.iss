; TinyTrayMonitor installer (Inno Setup 6).
;
; 사용자 단위 설치가 기본이다. 트레이 앱은 HKCU에 설정을 쓰므로 per-machine
; 설치와 섞이지 않는 편이 안전하고, UAC 프롬프트도 없다.
;
; 빌드는 build-installer.ps1이 담당한다. 다음 값은 명령줄에서 온다.
;   AppVersion, SourceExe, SourceRoot, OutputDir

#ifndef AppVersion
  #error AppVersion is required (build-installer.ps1 passes it)
#endif
#ifndef SourceExe
  #error SourceExe is required
#endif
#ifndef SourceRoot
  #error SourceRoot is required
#endif
#ifndef OutputDir
  #error OutputDir is required
#endif

; v0.11.0에서 이름을 TinyTrayMonitor / tiny-tray-monitor.exe로 바꿨다. 트레이 고정은
; 아이콘 GUID에 묶여 있어 exe 이름이 바뀌어도 유지된다(docs/SPIKE.md §18).
; 업그레이드는 이전 설치 폴더를 그대로 쓰므로(UsePreviousAppDir) 옛 사용자의 폴더는
; `tray-monitor`로 남고, 새 설치만 `TinyTrayMonitor` 폴더에 들어간다.
#define AppDisplayName "TinyTrayMonitor"
#define AppName "tiny-tray-monitor"
#define AppExeName "tiny-tray-monitor.exe"
#define AppPublisher "tinyrack"
#define AppUrl "https://github.com/tinyrack-net/tiny-tray-monitor"
; Run 값 이름. startup.rs의 VALUE_NAME과 같아야 한다.
#define RunValueName "TinyTrayMonitor"
; v0.11.0 이전 이름. 업그레이드에서 지우거나 옮긴다.
#define LegacyDisplayName "TinyTrayManager"
#define LegacyExeName "tray-monitor.exe"
#define LegacyRunValueName "tray-monitor"

[Setup]
; AppId는 절대 바꾸지 않는다. 바꾸면 업그레이드가 아니라 새 프로그램으로 설치된다.
AppId={{3E1D7B2A-5C4F-4A6B-9E2D-7F8A1C3B5D90}
AppName={#AppDisplayName}
AppVersion={#AppVersion}
AppVerName={#AppDisplayName} {#AppVersion}
AppPublisher={#AppPublisher}
AppPublisherURL={#AppUrl}
AppSupportURL={#AppUrl}
AppUpdatesURL={#AppUrl}/releases
DefaultDirName={autopf}\{#AppDisplayName}
DefaultGroupName={#AppDisplayName}
DisableProgramGroupPage=yes
LicenseFile={#SourceRoot}\LICENSE
OutputDir={#OutputDir}
OutputBaseFilename={#AppName}-{#AppVersion}-windows-x64-setup
SetupIconFile={#SourceRoot}\resources\app.ico
UninstallDisplayIcon={app}\{#AppExeName}
Compression=lzma2/max
SolidCompression=yes
WizardStyle=modern
PrivilegesRequired=lowest
PrivilegesRequiredOverridesAllowed=commandline dialog
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
MinVersion=10.0.17763
; 종료는 PrepareToInstall에서 IPC 창에 WM_CLOSE를 보내 처리한다. Restart Manager가
; 숨은 창을 찾지 못해도 동작하도록 자동 종료는 끈다.
CloseApplications=no
RestartApplications=no
AllowNoIcons=yes
ShowLanguageDialog=auto
; checkedonce(자동 시작 기본 체크)가 이전 설치의 작업 선택을 읽어야 동작한다. 기본값이지만
; 의존성을 분명히 하려고 적어 둔다.
UsePreviousTasks=yes

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"
Name: "korean"; MessagesFile: "compiler:Languages\Korean.isl"

[CustomMessages]
english.Autostart=Run at sign-in
korean.Autostart=로그인할 때 실행
english.StartupGroup=Startup
korean.StartupGroup=시작 프로그램

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"; Flags: unchecked
; 새 설치에서만 기본 체크된다. 이전 버전이 설치되어 있으면 checkedonce가 체크를 풀어,
; 업그레이드가 앱 설정에서 바꾼 자동 시작 선택을 덮어쓰지 않는다.
Name: "autostart"; Description: "{cm:Autostart}"; GroupDescription: "{cm:StartupGroup}"; Flags: checkedonce

[InstallDelete]
; v0.11.0 이전 설치가 남긴 바로 가기. 남겨 두면 시작 메뉴에 옛 이름이 함께 보인다.
; 옛 exe는 종료 직후 잠깐 잠겨 있어 여기서는 지워지지 않는다(실측). DeleteLegacyExe가
; 설치 뒤에 다시 시도하며 지운다.
Type: files; Name: "{autoprograms}\{#LegacyDisplayName}.lnk"
Type: files; Name: "{autodesktop}\{#LegacyDisplayName}.lnk"

[Files]
Source: "{#SourceExe}"; DestDir: "{app}"; DestName: "{#AppExeName}"; Flags: ignoreversion
Source: "{#SourceRoot}\README.md"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#SourceRoot}\LICENSE"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{autoprograms}\{#AppDisplayName}"; Filename: "{app}\{#AppExeName}"
Name: "{autodesktop}\{#AppDisplayName}"; Filename: "{app}\{#AppExeName}"; Tasks: desktopicon

[Registry]
; 새 설치에서 autostart 작업이 선택되어 있으면 앱과 같은 형식(따옴표로 감싼 exe 경로)으로
; Run 값을 넣는다. startup.rs의 is_enabled_named가 이 문자열을 그대로 비교한다.
Root: HKCU; Subkey: "Software\Microsoft\Windows\CurrentVersion\Run"; ValueType: string; ValueName: "{#RunValueName}"; ValueData: """{app}\{#AppExeName}"""; Flags: uninsdeletevalue; Tasks: autostart

[Run]
Filename: "{app}\{#AppExeName}"; Description: "{cm:LaunchProgram,{#AppDisplayName}}"; Flags: nowait postinstall skipifsilent runasoriginaluser

[Code]
const
  WM_CLOSE = $0010;
  WM_COMMAND = $0111;
  { IPC 창 클래스는 이름 변경 전 그대로다(ipc.rs). 그래서 옛 버전도 이 창으로 종료된다. }
  IPC_CLASS = 'tray-monitor-ipc';
  SETTINGS_CAPTION = '{#AppDisplayName} settings';
  LEGACY_SETTINGS_CAPTION = '{#LegacyDisplayName} settings';
  RUN_KEY = 'Software\Microsoft\Windows\CurrentVersion\Run';

function FindWindowW(lpClassName, lpWindowName: string): HWND;
  external 'FindWindowW@user32.dll stdcall';
function PostMessageW(hWnd: HWND; Msg: UINT; wParam, lParam: Longint): Boolean;
  external 'PostMessageW@user32.dll stdcall';

{ 실행 중인 인스턴스에 정상 종료를 요청한다. 설정 창이 떠 있으면 그 창도 닫아
  모달 루프가 메인 루프의 종료 검사를 막지 않게 한다. }
procedure RequestClose();
var
  Hwnd: HWND;
begin
  Hwnd := FindWindowW(IPC_CLASS, '');
  if Hwnd <> 0 then
    PostMessageW(Hwnd, WM_CLOSE, 0, 0);

  Hwnd := FindWindowW('#32770', SETTINGS_CAPTION);
  if Hwnd <> 0 then
    PostMessageW(Hwnd, WM_CLOSE, 0, 0);
  Hwnd := FindWindowW('#32770', LEGACY_SETTINGS_CAPTION);
  if Hwnd <> 0 then
    PostMessageW(Hwnd, WM_CLOSE, 0, 0);

  { 0.8.0 이하의 첫 실행 안내는 같은 제목의 MB_YESNO 상자로 떴고, MB_YESNO는
    WM_CLOSE를 무시하므로 IDNO(7)를 먼저 보낸다. 지금은 오류 상자만 같은 제목을 쓴다. }
  Hwnd := FindWindowW('#32770', '{#AppDisplayName}');
  if Hwnd = 0 then
    Hwnd := FindWindowW('#32770', '{#LegacyDisplayName}');
  if Hwnd = 0 then
    Hwnd := FindWindowW('#32770', 'tray-monitor');
  if Hwnd <> 0 then
  begin
    PostMessageW(Hwnd, WM_COMMAND, 7, 0);
    PostMessageW(Hwnd, WM_CLOSE, 0, 0);
  end;
end;

function PrepareToInstall(var NeedsRestart: Boolean): String;
var
  I: Integer;
begin
  Result := '';
  RequestClose();

  { 프로세스가 끝나면 IPC 창이 사라진다. 최대 5초까지 기다린다. }
  for I := 1 to 50 do
  begin
    if FindWindowW(IPC_CLASS, '') = 0 then
      Exit;
    Sleep(100);
  end;

  Result := '{#AppDisplayName} is still running. Close it from the tray menu and run Setup again.';
end;

{ v0.11.0 이전의 Run 값(tray-monitor)은 옛 exe를 가리킨다. 자동 시작을 켜 둔 사용자라는
  뜻이므로 새 이름과 새 exe 경로로 옮긴다. 업그레이드에서는 autostart 작업이 체크되지 않아
  Registry 섹션이 값을 쓰지 않으므로 여기서 직접 쓴다. }
{ 옛 exe를 지운다. 방금 종료된 옛 프로세스가 이미지 파일을 잠깐 쥐고 있으므로 최대 5초
  동안 다시 시도한다. }
procedure DeleteLegacyExe();
var
  Path: String;
  I: Integer;
begin
  Path := ExpandConstant('{app}\{#LegacyExeName}');
  for I := 1 to 50 do
  begin
    if not FileExists(Path) then
      Exit;
    if DeleteFile(Path) then
    begin
      Log('Deleted the old executable ' + Path);
      Exit;
    end;
    Sleep(100);
  end;
  Log('Could not delete the old executable ' + Path);
end;

procedure CurStepChanged(CurStep: TSetupStep);
var
  Old: String;
begin
  if CurStep <> ssPostInstall then
    Exit;
  DeleteLegacyExe();
  if not RegQueryStringValue(HKCU, RUN_KEY, '{#LegacyRunValueName}', Old) then
    Exit;
  RegWriteStringValue(HKCU, RUN_KEY, '{#RunValueName}', '"' + ExpandConstant('{app}\{#AppExeName}') + '"');
  RegDeleteValue(HKCU, RUN_KEY, '{#LegacyRunValueName}');
end;

{ 설치가 만든 값은 [Registry]의 uninsdeletevalue가 지운다. 여기서는 사용자가 앱 설정에서
  직접 켠 경우까지 포함해 남은 값을 확실히 지운다. }
procedure CurUninstallStepChanged(CurUninstallStep: TUninstallStep);
begin
  if CurUninstallStep = usUninstall then
  begin
    RegDeleteValue(HKCU, RUN_KEY, '{#RunValueName}');
    RegDeleteValue(HKCU, RUN_KEY, '{#LegacyRunValueName}');
  end;
end;
