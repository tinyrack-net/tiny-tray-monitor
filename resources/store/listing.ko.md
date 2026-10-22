# Store listing copy (ko-KR)

Partner Center → Store listing → 한국어(대한민국). 영어 `listing.md`와 내용을 맞춘다.

**Product name:** TinyTrayMonitor

**Short description:** CPU, 메모리, 네트워크, GPU, 디스크 상태를 Windows 시스템 트레이에 표시합니다.

**Keywords:** 트레이, CPU, 메모리, 네트워크, GPU, 디스크, 시스템 모니터

**Screenshots:** `screenshots/ko/01-settings.png`, `screenshots/ko/02-tray.png`

## Description

TinyTrayMonitor는 Windows 시스템 트레이에서 PC 상태를 보여 줍니다. 작은 지표 8개가 지금 일어나는 일을 바로 알려 줍니다.

- CPU 사용률
- 메모리 사용률
- 네트워크 다운로드
- 네트워크 업로드
- GPU 사용률
- GPU VRAM 사용률
- 디스크 사용량
- 디스크 로드율

지표마다 색으로 구분된 트레이 아이콘이 하나씩 있어, 작업 관리자를 열지 않고도 숫자를 한눈에 읽을 수 있습니다.

### 기능

- 독립된 지표 8개. 필요 없는 지표는 끌 수 있습니다.
- 측정할 그래픽 카드, 네트워크 어댑터, 드라이브를 고를 수 있습니다.
- 밝게, 어둡게, 자동 설정 창.
- 지표별 색상 또는 작업 표시줄 테마.
- 갱신 주기 1초부터.
- "로그인할 때 실행"이 기본으로 켜져 있어 재부팅 후에도 지표가 바로 보입니다. 설정에서 끌 수 있습니다.
- 가상 어댑터(VPN, Hyper-V 스위치)를 제외해 실제 네트워크 카드 수치만 볼 수 있습니다.
- 설정 창에서 지표를 작업 표시줄에 고정하는 방법을 안내하고, 작업 표시줄 설정을 바로 여는 버튼을 제공합니다.
- 계정, 원격 분석, 네트워크 접속이 없습니다. 설정과 로그는 PC에만 남습니다.
- 작고 네이티브합니다. Win32와 GDI로 만들었고 Electron이나 백그라운드 서비스가 없습니다.

TinyTrayMonitor는 오픈 소스(MIT)입니다. Microsoft Store 버전은 Windows 패키지 ID와 시작 작업 API를 사용하며, 업데이트 후에도 트레이 배치를 유지합니다.

Windows 10 버전 1809 이상이 필요합니다.

## What's new (0.12.0)

- 앱의 모든 부분이 TinyTrayMonitor 이름을 씁니다. 로그인 시 실행 항목도 포함됩니다.
- 이번 업데이트 후 로그인할 때 지표가 뜨지 않으면 설정에서 "로그인할 때 실행"을 한 번 다시 켜 주세요.

## Product features

- CPU, 메모리, 네트워크, GPU, VRAM, 디스크 지표
- 켜고 끌 수 있는 독립된 지표 8개
- 측정할 그래픽 카드, 네트워크 어댑터, 드라이브 선택
- 지표별 색상 또는 작업 표시줄 테마, 밝게·어둡게 창
- 갱신 주기 1초부터
- 로그인할 때 실행이 기본으로 켜짐
- VPN, Hyper-V 같은 가상 어댑터 제외
- 계정, 원격 분석, 네트워크 접속 없음
- 네이티브 Win32와 GDI, Electron 없음, 오픈 소스(MIT)
