<div align="center">

# tray-monitor

**CPU, memory and network readouts in the Windows system tray.**

[![CI](https://github.com/tinyrack-net/tray-monitor/actions/workflows/pipeline.yml/badge.svg)](https://github.com/tinyrack-net/tray-monitor/actions/workflows/pipeline.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

</div>

tray-monitor puts three live readouts in the taskbar notification area, each showing
what it measures and its current value:

```
 NET   MEM   CPU
 98K   42%   23%
```

No administrator rights, no kernel driver, no background service. It reads CPU time
counters, `GlobalMemoryStatusEx` and the interface table directly.

## Install

Download `tray-monitor-<version>-windows-x64.zip` from
[Releases](https://github.com/tinyrack-net/tray-monitor/releases), unpack it anywhere and run
`tray-monitor.exe`.

### The readouts start hidden

Windows puts **new** tray icons in the overflow area (`^`) instead of the taskbar. On the
first run tray-monitor says so and offers to open the setting. You can also do it yourself:

- Settings > Personalisation > Taskbar > **Other system tray icons** > turn on tray-monitor
- or drag the icons out of the `^` overflow

tray-monitor remembers your choice. Keep the executable where it is: the setting is tied to
the executable path, so moving the file makes Windows treat it as a new icon.

## Menu

Right-click any of the three icons.

| Item | What it does |
| --- | --- |
| CPU / Memory / Network | Show or hide that readout. The last visible one cannot be turned off |
| Run at sign-in | Adds or removes the `HKCU\...\Run` entry for the current executable |
| Count virtual adapters | Include Hyper-V, VPN and similar adapters in the network total |
| Open config file | Opens `config.toml` in your default editor |
| Exit | Stops tray-monitor |

Hovering an icon shows the exact numbers, including upload rate and memory in GB.

## Configuration

`%APPDATA%\tray-monitor\config.toml` is created on first run.

```toml
poll_interval_ms = 1000          # 250 ~ 10000
exclude_virtual_adapters = true  # skip adapters that mirror physical traffic
color_mode = "metric"            # "metric" = one colour per readout, "theme" = single colour

[show]
cpu = true
memory = true
network = true
```

Deleted or broken files fall back to the defaults; a broken file is kept as `config.toml.bak`.
Logs are written to `%LOCALAPPDATA%\tray-monitor\log.txt`.

## Command line

| Command | What it does |
| --- | --- |
| `tray-monitor` | Run the tray app |
| `tray-monitor --probe` | Print the metrics every second, without any tray icon |
| `tray-monitor --interfaces` | List network interfaces and mark which ones are counted |
| `tray-monitor --dump` | Render the icons and print the layout measurements |
| `tray-monitor --help` | Usage |

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
| Network | `GetIfTable2` byte counters, loopback and tunnel interfaces excluded |

Values are formatted to fit a square icon and rendered with GDI into a 32bpp DIB, downsampled
from an 8x supersample, then handed to `Shell_NotifyIcon` as an `HICON`. A 16px icon (100%
display scaling) leaves room for a 5px label and a 9px value, which is why the values are
never longer than four characters. Measurements and the reasoning behind the layout are in
[docs/SPIKE.md](docs/SPIKE.md).

Idle cost measured on a 2560x1440 100% display: about **11 MB** of memory and **0.4%** of one
CPU core at a one second interval.

## Limitations

- The taskbar decides the order of the three icons. Right-click the taskbar and use
  *Taskbar settings* or drag them to reorder.
- The icons are rendered for the display scaling that was active at startup. Changing the
  scaling keeps the existing bitmaps; the taskbar scales them.
- Requires Windows 11 or Windows 10 (build 17063 or later for `GetIfTable2`).
- CPU temperature, fan speed and voltages are not included: they need administrator rights
  and a kernel driver.

## Development

```
make validate    # cargo fmt --check, clippy -D warnings, cargo test
make run         # run the tray app
make interfaces  # network interface diagnosis
make dump        # icon layout measurement + BMP dump, then a comparison sheet
```

Layout changes should be checked visually. `make dump` writes `out/<theme>_px<size>_<metric>@16x.bmp`
and `tools/icon_sheet.py` joins them into one sheet per icon size.

## License

MIT
