# tray-monitor

## Project Overview

**tray-monitor** shows CPU, memory and network readouts in the Windows system tray.

- **Main technologies:** Rust (edition 2024), `windows`, `tray-icon`, `windows-registry`,
  `serde` + `toml`. All dependencies come from crates.io; there are no git dependencies.
- **Architecture:** a single binary crate rooted at the repository (`src`, `tools`, `docs`).
  There is no workspace and no library target: the modules under `src` are wired together by
  `main.rs` and exercised by `#[cfg(test)]` modules in place.
- **Platform:** Windows only. Metrics come from Win32 APIs and icons are rendered with GDI.

## Mandatory Validation Loop

Run the validation loop after changes:

- **All Rust checks:** `make validate` (`cargo fmt --check`, `cargo clippy -- -D warnings`,
  `cargo test`). CI runs the same three commands as separate jobs.
- **Icon or layout changes:** `make dump` regenerates icon renderings and prints the measured
  glyph heights; `tools/icon_sheet.py` joins them into a comparison sheet. Judge the sheet
  before and after the change, at every icon size and on both taskbar backgrounds.

## Project Structure

| Path | Contents |
| --- | --- |
| `src/main.rs` | Startup: DPI awareness, single instance, message loop |
| `src/app.rs` | State and the update cycle |
| `src/runtime.rs` | The `App` in thread-local storage, shared by the timer and the message loop |
| `src/timer.rs` | Hidden window that owns the refresh timer |
| `src/metrics/` | CPU, memory and network sampling plus value formatting |
| `src/icon/` | GDI rasteriser, downsampling, layout policy and the glyph cache |
| `src/tray/` | Tray slots, the context menu and the first-run onboarding |
| `src/config.rs` | `config.toml` / `state.toml` |
| `src/theme.rs` | Taskbar theme detection and the colour palettes |
| `src/settings/` | The settings window: a pure model, the Win32 dialog and the icon preview |
| `src/ipc.rs` | Hidden window that hands a second launch over to the running instance |
| `src/win32.rs` | The remaining `unsafe` helpers |
| `src/devtools.rs` | `--probe`, `--interfaces`, `--dump` |
| `build.rs` | Compiles `resources/app.rc` so the exe carries `VERSIONINFO` and the manifest |
| `resources/` | `app.rc` (version info, manifest reference) and `app.manifest` (comctl32 v6, PerMonitorV2, asInvoker) |
| `docs/` | [PLAN.md](docs/PLAN.md), [SPIKE.md](docs/SPIKE.md) |
| `tools/` | Python helpers for icon sheets, screenshots, tray keys |

## Windows Details That Will Bite

Every item below was measured, not assumed. `docs/SPIKE.md` records the evidence.

1. **Never recreate a tray icon.** Windows assigns icon IDs in creation order and the tray
   "promote to taskbar" setting is tied to that ID. Create every icon in `Metric::ALL` order at
   startup and use `set_visible(false)` to hide one. When adding a metric, append it to
   `Metric::ALL`: inserting one shifts every later ID and silently moves the user's promoted
   icons to different readouts.
2. **A label gets five pixels at 16px.** Anything wider than three characters, or any glyph
   whose meaning lives in a thin stroke (arrows), is unreadable. Compare candidates with
   `make dump` and the sheet before choosing.
3. **Never pass an empty string to GDI.** `DrawTextW` dereferences the dangling pointer of an
   empty slice and the process dies. `icon::gdi::rasterize` rejects empty input.
4. **Read luminance from the red channel of a BI_RGB DIB.** GDI fills B, G and R and leaves the
   reserved byte at zero, so the alpha channel is always 0.
5. **Do not take the timer on the thread queue.** `SetTimer(None, ...)` posts `WM_TIMER` to the
   thread, and the modal loops of `TrackPopupMenu` and `MessageBox` drop those messages, which
   freezes the icons. `src/timer.rs` owns a message-only window instead.
6. **A timer can re-enter.** The same modal loops run nested message loops, so the tick callback
   runs again while the previous call is on the stack. `timer::dispatch_tick` takes the callback
   out of its cell before running it and skips a re-entrant call.
7. **Do not treat a failed `RefCell` borrow as a reason to exit.** `runtime::should_exit`
   returns false when the app is borrowed.
8. **Keep the executable path stable.** The tray setting is keyed on it, so an update that moves
   the binary resets the user's choice. Release archives extract in place.
9. **Cache glyph renderings.** Recreating the font and device context every second doubles CPU
   use. Values are a small finite set, so a per-string cache keeps the hit rate high.
10. **`rc.exe` has no default include path, and comctl32 v6 misreports its version.** Run
    outside a developer prompt, the resource compiler fails on `#include <windows.h>`
    (`RC1015`) unless the SDK include directories are passed; `embed-resource` fills
    `%INCLUDE%` for us, so the resource step needs no extra setup in `build.rs`. When checking
    that the manifest took effect, read the *path* of the loaded `comctl32.dll`: the WinSxS
    assembly is `...common-controls_..._6.0.<build>` even though its file version string still
    says `5.82`. `docs/SPIKE.md` §10.
11. **A modal window opened while `App` is borrowed freezes the tray.** `runtime::with` holds the
    app for the whole call, so a `MessageBox` raised inside it drops every timer tick until it
    closes, and re-entrant dialog messages silently no-op. The settings window is opened from
    the message loop with no borrow held, and a failed change is reported through the window's
    status line instead of a `MessageBox`. `docs/SPIKE.md` §11-4.
12. **A read-only registry handle cannot delete a value.** `CURRENT_USER.open()` hands back a
    key opened for reading, so `remove_value` fails with `Access is denied`. Use `create()` for
    the removal path too.

## Merging

- `main` is protected and takes no direct pushes, admins included. Every change lands through
  a pull request.
- The one required check is `Quality Gate`, an aggregate job in `.github/workflows/pipeline.yml`
  that fails unless every quality job reports `success`. A job it needs that reports `skipped`
  fails the gate too, so anything added to `needs` has to run on both `pull_request` and
  `merge_group`.
- `gh pr merge --auto --squash <number>` queues the merge. Once the checks pass, the merge queue
  re-runs the pipeline on the pull request rebased onto current `main`, then squash-merges and
  deletes the branch.

## Release

- The version is single-sourced in `Cargo.toml`.
- Bump, commit and tag manually, then push the tag:

  ```
  git tag v0.1.0
  git push origin v0.1.0
  ```

- The `release` job in `.github/workflows/pipeline.yml` builds the Windows binary, packages a zip
  with `README.md` and `LICENSE`, writes `SHA256SUMS` and publishes a GitHub release with
  generated notes.
- Assume the archive is extracted in place: an update must not change the path of
  `tray-monitor.exe`, or every user loses their tray setting.

## Testing

- Pure logic (value formatting, delta arithmetic, layout selection, config parsing, downsampling
  and glyph caching) is unit tested with `cargo test` and needs no display.
- Icon rendering is tested with invariants (ink stays inside the box, glyph heights match the
  layout policy, GDI handles do not leak) rather than pixel hashes: anti-aliasing differs between
  Windows versions.
- The tray itself has no automated test. Use `make run`, then check the readouts, the context
  menu and `%LOCALAPPDATA%\tray-monitor\log.txt`. The log records a heartbeat every 60 updates
  with the current values and the glyph cache hit rate.
