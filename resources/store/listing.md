# Store listing copy

Copy for Partner Center → Store listing. Keep it in sync with the app.

**Product name:** TinyTrayManager

**Short description:** CPU, memory, network and GPU readouts in your Windows system tray.

**Keywords:** tray, cpu, memory, network, gpu, vram, system monitor, taskbar, performance

**Copyright:** Copyright (c) 2026 winetree94

**Support:** https://tinyrack.net/support/

**Privacy policy:** https://tinyrack.net/privacy/

**Website:** https://github.com/tinyrack-net/tray-monitor

**Category:** Utilities & tools

**System requirements:** Windows 10 version 1809 (build 17763) or later, x64.

## Description

TinyTrayManager keeps an eye on your PC from the Windows system tray. Six compact readouts show what is happening right now:

- CPU usage
- Memory usage
- Network download
- Network upload
- GPU usage
- GPU VRAM usage

Each readout is a single tray icon with a coloured value, so you can read the numbers at a glance without opening Task Manager.

### Features

- Six independent readouts. Turn off the ones you do not need.
- Per-readout colours, or follow the taskbar theme.
- Update interval from one second up.
- "Run at sign-in" is on by default, so the readouts are there after a reboot. Turn it off in Settings.
- Option to ignore virtual adapters (VPNs, Hyper-V switches) so the numbers match the physical NIC.
- A hint in the settings window that shows where to pin the readouts, with a button that opens the
taskbar settings.
- No account, no telemetry, no network access. Settings and logs stay on your PC.
- Small and native: Win32 and GDI, no Electron, no background service.

TinyTrayManager is open source (MIT). The Microsoft Store build uses the Windows package identity and startup-task APIs, and keeps your tray layout across updates.

Requires Windows 10 version 1809 or later.

## What's new (0.10.1)

- The readouts stay sharp when you change the display scale while the app is running. Before, they
kept their old size and Windows stretched them until the app was restarted.
- At 200% and above the digits are drawn at a whole multiple, so every stroke has the same width
(at 225%, for example, they are drawn at 2x with a small margin instead of being stretched to 2.25x).
