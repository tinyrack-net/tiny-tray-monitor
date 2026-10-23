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

**Pricing:** Free; a scheduled change makes it paid (`Tier1022`, USD 1.99) on 2027-01-01 in every market
(Partner Center → Pricing and availability, time zone *Local*). The listing copy does not mention it.

## Description

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
- The network total counts physical adapters only, so VPNs and Hyper-V switches do not count the same traffic twice.
- A hint in the settings window that shows where to pin the readouts, with a button that opens the
taskbar settings.
- No account, no telemetry, no network access. Settings and logs stay on your PC.
- Small and native: Win32 and GDI, no Electron, no background service.

TinyTrayMonitor is open source (MIT). The Microsoft Store build uses the Windows package identity and startup-task APIs, and keeps your tray layout across updates.

Requires Windows 10 version 1809 or later.

## What's new (0.13.0)

- The "Count virtual adapters" setting is gone. "All adapters" now always sums the physical network adapters, so VPNs and Hyper-V switches no longer inflate the total.
- To watch a VPN or virtual adapter, pick it as the network device in Settings.

## Product features

- CPU, memory, network, GPU, VRAM and disk readouts in the tray
- Eight independent readouts you can turn on or off
- Choose the graphics card, network adapter and drive
- Per-readout colours or taskbar theme; light or dark window
- Update interval from one second
- Run at sign-in is on by default
- Network total counts physical adapters only
- No account, no telemetry, no network access
- Native Win32 and GDI, no Electron; open source (MIT)
