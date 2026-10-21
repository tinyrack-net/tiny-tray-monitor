; tray-monitor installer (Inno Setup 6).
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

; 표시 이름은 제품 이름을 따른다. 설치 경로와 exe 이름은 업데이트 호환을 위해
; `tray-monitor`로 유지한다(트레이 승격 설정이 실행 경로에 묶인다).
#define AppDisplayName "TinyTrayManager"
#define AppName "tray-monitor"
#define AppExeName "tray-monitor.exe"
#define AppPublisher "tinyrack"
#define AppUrl "https://github.com/tinyrack-net/tray-monitor"

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
DefaultDirName={autopf}\{#AppName}
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
Root: HKCU; Subkey: "Software\Microsoft\Windows\CurrentVersion\Run"; ValueType: string; ValueName: "{#AppName}"; ValueData: """{app}\{#AppExeName}"""; Flags: uninsdeletevalue; Tasks: autostart

[Run]
Filename: "{app}\{#AppExeName}"; Description: "{cm:LaunchProgram,{#AppDisplayName}}"; Flags: nowait postinstall skipifsilent runasoriginaluser

[Code]
const
  WM_CLOSE = $0010;
  WM_COMMAND = $0111;
  IPC_CLASS = 'tray-monitor-ipc';
  SETTINGS_CAPTION = 'TinyTrayManager settings';

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

  { 0.8.0 이하의 첫 실행 안내는 같은 제목의 MB_YESNO 상자로 떴고, MB_YESNO는
    WM_CLOSE를 무시하므로 IDNO(7)를 먼저 보낸다. 지금은 오류 상자만 같은 제목을 쓴다. }
  Hwnd := FindWindowW('#32770', '{#AppDisplayName}');
  if Hwnd = 0 then
    Hwnd := FindWindowW('#32770', '{#AppName}');
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

{ 설치가 만든 값은 [Registry]의 uninsdeletevalue가 지운다. 여기서는 사용자가 앱 설정에서
  직접 켠 경우까지 포함해 남은 값을 확실히 지운다. }
procedure CurUninstallStepChanged(CurUninstallStep: TUninstallStep);
begin
  if CurUninstallStep = usUninstall then
    RegDeleteValue(HKCU, 'Software\Microsoft\Windows\CurrentVersion\Run', '{#AppName}');
end;
