# Store listing copy

Copy for Partner Center → Store listing → English (United States). The Korean and Japanese
listings are in `listing.ko.md` and `listing.ja.md`; keep all three in step.

**Product name:** TinyTrayMonitor

**Short description:** CPU, memory, network, GPU and disk readouts in your Windows system tray.

**Keywords:** tray, cpu, memory, network, gpu, vram, disk, system monitor, taskbar, performance

**Copyright:** Copyright (c) 2026 winetree94

**Support:** https://tinyrack.net/support/

**Privacy policy:** https://tinyrack.net/privacy/

**Website:** https://github.com/tinyrack-net/tiny-tray-monitor

**Category:** Utilities & tools

**System requirements:** Windows 10 version 1809 (build 17763) or later, x64.

**Screenshots:** `screenshots/en/01-settings.png`, `screenshots/en/02-tray.png`

**Pricing:** Free until 2026-12-31; a scheduled change makes it paid (`Tier1022`, USD 1.99) on
2027-01-01 in every market. Remove the first paragraph of each description after that date.

## Description

Free until December 31, 2026. From January 1, 2027 it becomes a paid app; if you get it before then, you keep it and its updates for free.

TinyTrayMonitor keeps an eye on your PC from the Windows system tray. Eight compact readouts show what is happening right now:

- CPU usage
- Memory usage
- Network download
- Network upload
- GPU usage
- GPU VRAM usage
- Disk usage
- Disk activity

Each readout is a single tray icon with a coloured value, so you can read the numbers at a glance without opening Task Manager.

### Features

- Eight independent readouts. Turn off the ones you do not need.
- Choose the graphics card, network adapter and drive to measure.
- Light, dark or automatic settings window.
- Per-readout colours, or follow the taskbar theme.
- Update interval from one second up.
- "Run at sign-in" is on by default, so the readouts are there after a reboot. Turn it off in Settings.
- Option to ignore virtual adapters (VPNs, Hyper-V switches) so the numbers match the physical NIC.
- A hint in the settings window that shows where to pin the readouts, with a button that opens the
taskbar settings.
- No account, no telemetry, no network access. Settings and logs stay on your PC.
- Small and native: Win32 and GDI, no Electron, no background service.

TinyTrayMonitor is open source (MIT). The Microsoft Store build uses the Windows package identity and startup-task APIs, and keeps your tray layout across updates.

Requires Windows 10 version 1809 or later.

## What's new (0.12.0)

- Every part of the app now uses the TinyTrayMonitor name, including its startup entry.
- If the readouts no longer start at sign-in after this update, turn on "Run at sign-in" in Settings once.
