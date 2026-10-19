<div align="center">

# tray-monitor

**CPU, memory and network readouts in the Windows system tray.**

[![CI](https://github.com/tinyrack-net/tray-monitor/actions/workflows/pipeline.yml/badge.svg)](https://github.com/tinyrack-net/tray-monitor/actions/workflows/pipeline.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

</div>

tray-monitor puts four live readouts in the taskbar notification area, each showing
what it measures and its current value:

```
 UP    DN    MEM   CPU
341K  1.7K   41%   34%
```

No administrator rights, no kernel driver, no background service. It reads CPU time
counters, `GlobalMemoryStatusEx` and the interface table directly.

## Install

### Installer (recommended)

Download `tray-monitor-<version>-windows-x64-setup.exe` from
[Releases](https://github.com/tinyrack-net/tray-monitor/releases) and run it. The default is a
per-user install under `%LOCALAPPDATA%\Programs\tray-monitor`, so no UAC prompt appears and the
install path stays the same across upgrades. `/ALLUSERS` (or the wizard) can install it for every
user instead. Uninstalling keeps `%APPDATA%\tray-monitor`.

### Portable

Download `tray-monitor-<version>-windows-x64.zip`, unpack it anywhere and run
`tray-monitor.exe`.

### MSIX (Store build)

Releases also contain `tray-monitor-<version>-windows-x64.msix`. It is **unsigned**, so installing
it yourself requires developer mode (`Add-AppxPackage -Register`); the Store signs the submitted
package. A packaged build keeps its config, state and log in
`%LOCALAPPDATA%\Packages\Tinyrack.TrayMonitor_<hash>\LocalState`, uses the Windows startup-task API
instead of the `Run` key, and can lose the tray promotion when the package updates. See
[docs/PACKAGING.md](docs/PACKAGING.md).

### The readouts start hidden

Windows puts **new** tray icons in the overflow area (`^`) instead of the taskbar. On the
first run tray-monitor says so and offers to open the setting. You can also do it yourself:

- Settings > Personalisation > Taskbar > **Other system tray icons** > turn on tray-monitor
- or drag the icons out of the `^` overflow

tray-monitor remembers your choice. Keep the executable where it is: the setting is tied to
the executable path, so moving the file makes Windows treat it as a new icon.

## Menu

Right-click any of the icons.

| Item | What it does |
| --- | --- |
| Settings… | Opens the settings window |
| CPU / Memory / Network down (DN) / Network up (UP) | Show or hide that readout. The last visible one cannot be turned off |
| Run at sign-in | Adds or removes the `HKCU\...\Run` entry (or the package startup task) |
| Count virtual adapters | Include Hyper-V, VPN and similar adapters in the network total |
| Open config file | Opens `config.toml` in your default editor |
| Exit | Stops tray-monitor |

Hovering an icon shows the exact numbers, including upload rate and memory in GB.

## Settings window

Everything the menu does, plus the update interval, in one window. Re-running
`tray-monitor.exe` (or running `tray-monitor --settings`) opens it in the instance that is
already running.

| Setting | Notes |
| --- | --- |
| Readouts | CPU, memory, network down and up. The last one cannot be turned off, and the preview shows what the tray icons look like right now |
| Update every | 250 ms to 10 s. Pick a preset or type an exact number of milliseconds |
| Colours | One colour per readout, or a single colour from the taskbar theme |
| Network | Count virtual adapters (Hyper-V, VPN, vSwitch) in the network total |
| Startup | Run at sign-in |
| Buttons | Open `config.toml`, open the log file, reset everything in `config.toml` to the defaults |

There is no OK button: every change applies immediately and is written to `config.toml` at
once, so the tray icons are the preview. Close the window with the button or `Escape`.

With a dark taskbar the window opens dark, and the tray menu follows the same setting.

## Configuration

`%APPDATA%\tray-monitor\config.toml` is created on first run. In the MSIX package the file lives
in `%LOCALAPPDATA%\Packages\Tinyrack.TrayMonitor_<hash>\LocalState\config.toml`; an existing
config from `%APPDATA%\tray-monitor` is copied there on the first packaged run.

```toml
poll_interval_ms = 1000          # 250 ~ 10000
exclude_virtual_adapters = true  # skip adapters that mirror physical traffic
color_mode = "metric"            # "metric" = one colour per readout, "theme" = single colour

[show]
cpu = true
memory = true
network_down = true
network_up = true
```

A `network = ...` setting written by v0.1.0 is migrated to `network_down` and
`network_up` on the next start.

Deleted or broken files fall back to the defaults; a broken file is kept as `config.toml.bak`.
Logs are written to `%LOCALAPPDATA%\tray-monitor\log.txt`.

## Command line

| Command | What it does |
| --- | --- |
| `tray-monitor` | Run the tray app |
| `tray-monitor --settings` | Run and open the settings window (or open it in the running instance) |
| `tray-monitor --probe` | Print the metrics every second, without any tray icon |
| `tray-monitor --interfaces` | List network interfaces and mark which ones are counted |
| `tray-monitor --dump` | Render the icons and print the layout measurements |
| `tray-monitor --theme` | Print the taskbar theme and whether the dark mode API is available |
| `tray-monitor --help` | Usage |

The settings window is also reachable from the tray menu; `--settings` is for scripts and
keyboard launchers.

`--interfaces` is the quickest way to understand the network number:

```
  idx    type   up     hardware  counted description
  7      6      true   true      true    Intel(R) I211 Gigabit Network Connection (Ethernet)
  16     71     true   true      true    Intel(R) Wi-Fi 6 AX200 160MHz (Wi-Fi)
  12     6      true   false     false   Hyper-V Virtual Switch Extension Adapter (vSwitch ...)
```

## How it works

| Metric | Source |
| --- | --- |
| CPU | `GetSystemTimes` deltas (kernel time includes idle time) |
| Memory | `GlobalMemoryStatusEx` |
| Network down / up | `GetIfTable2` byte counters, loopback and tunnel interfaces excluded |

Values are formatted to fit a square icon and rendered with GDI into a 32bpp DIB, downsampled
from an 8x supersample, then handed to `Shell_NotifyIcon` as an `HICON`. A 16px icon (100%
display scaling) leaves room for a 5px label and a 9px value, which is why the values are
never longer than four characters. Measurements and the reasoning behind the layout are in
[docs/SPIKE.md](docs/SPIKE.md).

Idle cost measured on a 2560x1440 100% display: about **19 MB** of working set (3 MB private,
216 handles, all steady) and **0.4%** of one CPU core at a one second interval.

The label row is 5px tall on a 16px icon, which is too small for an arrow: `↓` and `↑`
collapse into the same thin stroke. `DN` and `UP` stay readable, and the two readouts
also use different colours. Measurements for every candidate are in
[docs/PLAN.md](docs/PLAN.md).

## Limitations

- The taskbar decides the order of the icons. Drag them to reorder.
- Dark mode uses the unofficial `uxtheme` ordinals (133/135/136). If a future Windows build
  drops them, the window stays light; `tray-monitor --theme` says which case you are in.
- The icons are rendered for the display scaling that was active at startup. Changing the
  scaling keeps the existing bitmaps; the taskbar scales them.
- Requires Windows 11 or Windows 10 (build 17063 or later for `GetIfTable2`).
- CPU temperature, fan speed and voltages are not included: they need administrator rights
  and a kernel driver.
- The MSIX package is unsigned (developer mode or Store signing needed), and a package update
  changes the executable path, so Windows may hide the tray icons again. The onboarding prompt
  returns after an update.

## Development

```
make validate    # cargo fmt --check, clippy -D warnings, cargo test
make run         # run the tray app
make interfaces  # network interface diagnosis
make dump        # icon layout measurement + BMP dump, then a comparison sheet
make assets      # regenerate resources/app.ico and the MSIX tiles
make package     # build the installer and the MSIX into dist/
```

`make package` needs Inno Setup 6 and the Windows SDK (`makeappx.exe`). Without `make`, run the
scripts directly:

```
pwsh -File packaging/windows/build-installer.ps1 -Exe target/release/tray-monitor.exe
pwsh -File packaging/windows/build-msix.ps1 -Exe target/release/tray-monitor.exe
```

Layout changes should be checked visually. `make dump` writes `out/<theme>_px<size>_<metric>@16x.bmp`
and `tools/icon_sheet.py` joins them into one sheet per icon size.

## License

MIT
