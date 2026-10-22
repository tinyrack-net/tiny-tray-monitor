<div align="center">

# tray-monitor

**CPU, memory, network, GPU and disk readouts in the Windows system tray.**

[![CI](https://github.com/tinyrack-net/tray-monitor/actions/workflows/pipeline.yml/badge.svg)](https://github.com/tinyrack-net/tray-monitor/actions/workflows/pipeline.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

</div>

tray-monitor puts eight live readouts in the taskbar notification area, each showing
what it measures and its current value:

```
 DIO   DSK   VRM   GPU   ↑KB   ↓MB   MEM   CPU
 6     58    90    14    341   1.7   41    34
```

No administrator rights, no kernel driver, no background service. It reads CPU time
counters, `GlobalMemoryStatusEx`, the interface table, the GPU engine statistics
(`D3DKMTQueryStatistics`) and the disk performance counters (`IOCTL_DISK_PERFORMANCE`)
directly.

## Install

### Installer (recommended)

Download `tray-monitor-<version>-windows-x64-setup.exe` from
[Releases](https://github.com/tinyrack-net/tray-monitor/releases) and run it. The default is a
per-user install under `%LOCALAPPDATA%\Programs\tray-monitor`, so no UAC prompt appears and the
install path stays the same across upgrades. `/ALLUSERS` (or the wizard) can install it for every
user instead. Uninstalling keeps `%APPDATA%\tray-monitor`.

The wizard turns on **Run at sign-in** by default, so the readouts are back after a reboot.
Uncheck the task on the wizard page to skip it. An upgrade keeps whatever you chose afterwards.

### Portable

Download `tray-monitor-<version>-windows-x64.zip`, unpack it anywhere and run
`tray-monitor.exe`.

### MSIX (Store build)

Releases also contain `tray-monitor-<version>-windows-x64.msix`. It is **unsigned**: Windows will
not install the file directly. The Store signs the submitted package; for local testing, unpack it
and run `Add-AppxPackage -Register AppxManifest.xml` in developer mode. A packaged build keeps its
config and log in
`%LOCALAPPDATA%\Packages\Tinyrack.TinyTrayManager_<hash>\LocalState`, uses the Windows startup-task API
instead of the `Run` key, and keeps the tray promotion across package updates because each
readout registers with a fixed GUID that Windows keys the setting on. The package is enabled at
sign-in on first install, and turning it off in Settings or Task Manager survives later updates.
Launching it from the Start menu only starts the tray; run it again or use the tray menu to open
the settings.
See [docs/PACKAGING.md](docs/PACKAGING.md).

### The readouts start hidden

Windows puts **new** tray icons in the overflow area (`^`) instead of the taskbar, so nothing
shows up there on the first run. Pin them yourself:

- Settings > Personalisation > Taskbar > **Other system tray icons** > turn on tray-monitor
- or drag the icons out of the `^` overflow

The settings window (right-click a readout > **Settings…**) says this too and has an
**Open taskbar settings** button that opens the page for you. tray-monitor never reads the
setting; pinning is up to you. Each readout registers with a fixed GUID, so a package update
that moves the executable keeps your choice.

## Menu

Right-click any of the icons.

| Item | What it does |
| --- | --- |
| Settings… | Opens the settings window |
| CPU / Memory / Network down / Network up / GPU / GPU VRAM / Disk usage / Disk activity | Show or hide that readout. The last visible one cannot be turned off |
| Exit | Stops tray-monitor |

Hovering an icon shows the exact numbers, including upload rate, memory in GB and the disk
read and write rates.

## Settings window

Everything the menu does, plus the update interval, colours, the device to measure and startup,
in one window. The list on the left picks a page: **General** for the settings shared by every
readout, then one page each for **CPU**, **Memory**, **GPU**, **Network** and **Disk**. There is
only ever one settings window:

| Situation | `Settings…` in the tray menu | Running `tray-monitor.exe` again |
| --- | --- | --- |
| Settings window open | Brings it to the front | Brings it to the front |
| Settings window closed | Opens it | Opens it in the running instance |
| App not running | — | Starts the tray app without the settings window |

| Page | Setting | Notes |
| --- | --- | --- |
| General | Update every | 250 ms to 10 s. Pick a preset or type an exact number of milliseconds |
| General | Icon text | One colour per readout, or a single colour from the taskbar theme |
| General | Window | `Automatic` follows the taskbar; `Light` or `Dark` forces this window and the tray menu. The tray icons always follow the taskbar so they stay readable |
| General | Startup | Run at sign-in: adds or removes the `HKCU\...\Run` entry (or the package startup task). A new install turns it on; turn it off here |
| General | Language | `Automatic (system)`, English, 한국어 or 日本語. Applies at once to the window, the tray menu and the tooltips |
| CPU, Memory | Readouts | Show or hide the readout. The device line names the processor or the installed memory |
| GPU | Device | `Automatic` measures the adapter with the most dedicated memory; or pick one adapter |
| Network | Device | `All adapters` sums every counted adapter; or pick one, such as `Ethernet` or `Wi-Fi`. `Count virtual adapters` (Hyper-V, VPN, vSwitch) applies to the sum |
| Disk | Device | `Automatic` measures the drive Windows is installed on; or pick a drive. Activity is the busy time of the physical disk behind that drive |
| Every page | Readouts | The last visible readout cannot be turned off |
| Buttons | Reset to defaults | Reset everything in `config.toml` to the defaults |

There is no OK button: every change applies immediately and is written to `config.toml` at
once, so the tray icons are the preview. Close the window with the button or `Escape`.

A chosen device that disappears (a USB drive pulled out, a renamed adapter) is kept in the
settings and marked `(not found)`; the readout falls back to the automatic choice until the
device comes back.


## Configuration

`%APPDATA%\tray-monitor\config.toml` is created on first run. In the MSIX package the file lives
in `%LOCALAPPDATA%\Packages\tinyrack.TinyTrayManager_<hash>\LocalState\config.toml`; an existing
config from `%APPDATA%\tray-monitor` is copied there on the first packaged run.

```toml
poll_interval_ms = 1000          # 250 ~ 10000
exclude_virtual_adapters = true  # skip adapters that mirror physical traffic
color_mode = "metric"            # "metric" = one colour per readout, "theme" = single colour
ui_theme = "auto"                # window and tray menu: "auto" | "light" | "dark"
language = "auto"                # "auto" | "en" | "ko" | "ja"

[show]
cpu = true
memory = true
network_down = true
network_up = true
gpu = true
gpu_vram = true
disk_usage = true
disk_active = true

[devices]                        # empty = automatic
gpu = ""                         # adapter name, e.g. "NVIDIA GeForce RTX 4060"
network = ""                     # interface alias, e.g. "Ethernet" or "Wi-Fi"
disk = ""                        # drive, e.g. "D:"
```

A `network = ...` setting written by v0.1.0 is migrated to `network_down` and
`network_up` on the next start.

`language = "auto"` follows the Windows display language. Only English, Korean and Japanese
are bundled; any other language falls back to English. Logs are always written in English to
`%LOCALAPPDATA%\tray-monitor\log.txt`.

Deleted or broken files fall back to the defaults; a broken file is kept as `config.toml.bak`.

## Command line

| Command | What it does |
| --- | --- |
| `tray-monitor` | Run the tray app |
| `tray-monitor --settings` | Run and open the settings window at once (or show it in the running instance) |
| `tray-monitor --probe` | Print the metrics every second, without any tray icon |
| `tray-monitor --interfaces` | List network interfaces and mark which ones are counted |
| `tray-monitor --dump` | Render the icons and print the layout measurements |
| `tray-monitor --theme` | Print the taskbar theme and whether the dark mode API is available |
| `tray-monitor --dpi [--watch]` | Print the system and taskbar DPI and the icon size picked from them; `--watch` reprints on change |
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
| GPU | `D3DKMTQueryStatistics` per-engine running time deltas. The readout is the busiest engine of the adapter with the largest dedicated memory |
| GPU VRAM | The same call, summed over the adapter's non-aperture segments, against the DXGI dedicated limit |
| Disk usage | `GetDiskFreeSpaceExW` on the chosen drive |
| Disk activity | `IOCTL_DISK_PERFORMANCE` on the physical disk behind that drive, opened with no access rights: `1 - idle / elapsed`, the same figure as *Active time* in Task Manager. A volume that spans several disks shows the busiest one |

Values are formatted to fit a square icon and drawn with a built-in 1-bit pixel font
(a 5px caption over a 10px value) into an RGBA bitmap, then handed to `Shell_NotifyIcon` as an
`HICON`. There is no anti-aliasing and no GDI text: the glyph table is generated from
`tools/pixelfont.py`, so the same input always produces the same pixels. The 16px master is
scaled to the tray size with nearest-neighbour, which keeps the result 1-bit at every display
scale. The unit lives in the caption (`CPU`, `MEM`, `↓MB`, `↑KB`), so the value row only ever holds a
number of at most three characters and can use wide 4px digits with a 1px gap. Measurements and the
reasoning behind the layout are in
[docs/SPIKE.md](docs/SPIKE.md) and
[docs/PIXEL-FONT.md](docs/PIXEL-FONT.md).

Idle cost measured on a 2560x1440 100% display: about **19 MB** of working set (3 MB private,
216 handles, all steady) and **0.4%** of one CPU core at a one second interval.

The label row is 5px tall. `DN` and `UP` stay readable, and the two readouts also use
different colours; an arrow was rejected because two arrows are harder to tell apart than two
words at this size. Measurements for every candidate are in
[docs/PLAN.md](docs/PLAN.md) §11.

## Limitations

- The taskbar decides the order of the icons. Drag them to reorder.
- Dark mode uses the unofficial `uxtheme` ordinals (133/135/136). If a future Windows build
  drops them, the window stays light; `tray-monitor --theme` says which case you are in.
- The icons are rendered for the display scaling that was active at startup. Changing the
  scaling keeps the existing bitmaps; the taskbar scales them.
- Requires Windows 11 or Windows 10 (build 17063 or later for `GetIfTable2`).
- CPU temperature, fan speed and voltages are not included: they need administrator rights
  and a kernel driver.
- The GPU readout follows one adapter: by default the one with the largest dedicated memory.
  Pick another on the GPU page. The value is the busiest engine of that adapter, so it can
  differ from the per-engine graphs in Task Manager.
- The disk readouts follow one drive. Activity belongs to the whole physical disk, so two
  drives on the same disk show the same activity.
- The MSIX package is unsigned (developer mode or Store signing needed). A package update
  changes the executable path, but the fixed per-readout GUIDs keep the tray promotion. If the
  shell rejects a GUID, the app falls back to per-path registration, which Windows ties to the
  executable path.

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
