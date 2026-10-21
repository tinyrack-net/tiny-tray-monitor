# tray-monitor

## Project Overview

**tray-monitor** shows CPU, memory and network readouts in the Windows system tray.
The Microsoft Store listing is named **TinyTrayManager** (`tinyrack.TinyTrayManager`, Store ID
`9MT22CHX25FD`). The exe name, install path and config folders stay `tray-monitor` so existing
installs keep their tray promotion.

- **Main technologies:** Rust (edition 2024), `windows`, `windows-future`, `tray-icon`,
  `windows-registry`, `serde` + `toml`. All dependencies come from crates.io; there are no git
  dependencies.
- **Architecture:** a single binary crate rooted at the repository (`src`, `resources`,
  `packaging`, `tools`, `docs`).
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
- **Icon assets:** after editing `tools/make_assets.py`, run it and commit the results;
  `python tools/make_assets.py --check` is part of the package job.
- **Glyph table:** after editing `tools/pixelfont.py`, run it and commit `src/icon/glyphs.rs`;
  `python tools/pixelfont.py --check` is part of the package job.
- **Packaging changes:** `make package` and then run the installer and MSIX checks in
  [docs/PACKAGING.md](docs/PACKAGING.md) §5. CI builds both on every pull request.

## Project Structure

| Path | Contents |
| --- | --- |
| `src/main.rs` | Startup: DPI awareness, single instance, message loop |
| `src/app.rs` | State and the update cycle |
| `src/runtime.rs` | The `App` in thread-local storage, shared by the timer and the message loop |
| `src/timer.rs` | Hidden window that owns the refresh timer |
| `src/metrics/` | CPU, memory and network sampling plus value formatting |
| `src/icon/` | 1-bit glyph table, pixel renderer, layout policy and the glyph cache |
| `src/tray/` | Tray slots, the context menu and the first-run onboarding |
| `src/config.rs` | `config.toml` / `state.toml` |
| `src/i18n.rs` | The English / Korean / Japanese string tables and the current language |
| `src/platform.rs` | Package detection and the config/data locations (`LocalState` for MSIX) |
| `src/theme.rs` | Taskbar theme detection and the colour palettes |
| `src/settings/` | The settings window: a pure model, the Win32 dialog and the icon preview |
| `src/ipc.rs` | Hidden window that hands a second launch over to the running instance |
| `src/win32.rs` | The remaining `unsafe` helpers, including the unofficial dark-mode calls |
| `src/devtools.rs` | `--probe`, `--interfaces`, `--dump` |
| `build.rs` | Compiles `resources/app.rc` so the exe carries `VERSIONINFO`, the manifest and the icon |
| `resources/` | `app.rc`, `app.manifest`, `app.ico`, the MSIX tiles under `msix/`, and the Store listing assets under `store/` |
| `packaging/windows/` | Inno Setup script, MSIX manifest template and the PowerShell build scripts |
| `docs/` | [PLAN.md](docs/PLAN.md), [SPIKE.md](docs/SPIKE.md), [PACKAGING.md](docs/PACKAGING.md), [RELEASE-AUTOMATION.md](docs/RELEASE-AUTOMATION.md) |
| `tools/` | Python helpers for icon sheets, screenshots, tray keys and `make_assets.py` |

## Windows Details That Will Bite

Every item below was measured, not assumed. `docs/SPIKE.md` records the evidence.

1. **Never recreate a tray icon.** Windows assigns icon IDs in creation order and the tray
   "promote to taskbar" setting is tied to that ID. Create every icon in `Metric::ALL` order at
   startup and use `set_visible(false)` to hide one. When adding a metric, append it to
   `Metric::ALL`: inserting one shifts every later ID and silently moves the user's promoted
   icons to different readouts.
2. **The label row is five pixels at 16px.** Anything longer than three characters overflows
   the icon. Compare candidates with `make dump` and the sheet before choosing.
3. **Never pass an empty string to a renderer.** GDI `DrawTextW` dereferenced the dangling
   pointer of an empty slice and killed the process; the pixel renderer returns `None` instead.
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
9. **Cache glyph renderings.** The pixel renderer is cheap, but values are a small finite set, so
   a per-string cache avoids re-allocating a bitmap on every tick. The hit rate is in the log.
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
13. **Applying the theme sends theme messages again.** `SetPreferredAppMode` and
    `SetWindowTheme` re-post `WM_SETTINGCHANGE`/`WM_THEMECHANGED`; handling those messages by
    re-applying the theme recurses until the stack overflows (`c00000fd` in ntdll, measured on
    build 26300). `settings/dialog.rs` guards the re-entry with a thread-local flag.
14. **A package cannot write the Run key.** MSIX registry writes land in a virtual hive, so the
    `Run at sign-in` entry never reaches the real `HKCU`. `startup.rs` uses
    `StartupTask::RequestEnableAsync` when `platform::packaged()` is true.
15. **A package must not rely on `%APPDATA%`.** New folders are redirected to
    `LocalCache\Roaming`, while existing ones are not, so the location depends on install
    history. `platform.rs` writes config/state/log to `LocalState` and copies an existing
    `%APPDATA%\tray-monitor` config on the first packaged run.
16. **Do not call `StartupTask` from an input-synchronous handler.** Calling
    `RequestEnableAsync` inside `WM_COMMAND` fails with `RPC_E_CANTCALLOUT_ININPUTSYNCCALL`.
    The settings dialog posts `WM_APP_AUTOSTART` to itself and commits in that message instead.
17. **A package update changes the executable path.** `NotifyIconSettings` keys are per path, so
    the promotion used to reset after every version update. Each readout registers with a
    fixed per-metric GUID (`tray::TRAY_GUIDS`), and Windows keys the promotion on that GUID, so
    the setting survives a package update. The GUIDs and the `Metric::ALL` order must never
    change. Acceptance is decided by `NIM_ADD` plus the `NIF_GUID` tooltip round-trip only:
    **a missing `NotifyIconSettings` entry is not a refusal** (the shell never rewrites a key for
    an identity it already knows), and treating it as one made every second launch fall back to
    path registration. Compare paths after `\WindowsApps\` because packaged paths start with a
    known-folder GUID. See `docs/SPIKE.md` §15.
18. **A quit request must close modal dialogs first.** The installer posts `WM_CLOSE` to the IPC
    window, but the settings dialog or the onboarding `MB_YESNO` box keeps the main loop in a
    nested loop. `win32::window::close_dialogs` posts `WM_CLOSE` to every `#32770` window in the
    process, and `run_onboarding`/`settings::open` skip showing a new dialog when an exit is
    already pending.
19. **`tray-icon` 0.26.1 drops `NIF_GUID` from `set_tooltip`.** Every tooltip update fails with
    `E_FAIL` once an icon uses a GUID, while `set_icon` and `set_visible` still work.
    `win32::set_guid_tooltip` sends `NIM_MODIFY` with `NIF_TIP | NIF_GUID` directly, and `Tray`
    uses it whenever a slot has a GUID. The same call also serves as the first registration
    probe; re-check both when the crate is upgraded.
20. **`DarkMode_Explorer` has no radio button or group box text colour.** Its `Button` class
    defines `TMT_TEXTCOLOR` for checkboxes only, so themed radio buttons and group box
    captions draw black text on the dark dialog background. `win32::dark::paint_radio` gives
    radio buttons `DarkMode_DarkTheme` and falls back to turning the visual style off;
    `paint_group_box` turns the style off outright so the caption uses the
    `WM_CTLCOLORSTATIC` colour. `docs/SPIKE.md` §11-5, §13-6.
21. **Menu clicks must not wait for the main loop.** A modal loop (the settings window, a
    message box) stops the main loop, so events read from `MenuEvent::receiver()` piled up until
    the dialog closed. `main.rs` installs `MenuEvent::set_event_handler`, which calls
    `runtime::menu_event` from the tray window's `WM_COMMAND`; it queues the id only when `App`
    is borrowed, and the next message or timer tick drains it. `docs/SPIKE.md` §11-4.
22. **There is only one settings window.** A request while it is open must bring it to the
    front, never set the "open" flag: the main loop would reopen the window right after it
    closes. `App::request_settings` and `settings::open` both go through
    `settings::dialog::focus`. A second launch calls `AllowSetForegroundWindow` before posting
    to the IPC window, because the running instance is in the background.
23. **Every user-visible string lives in `src/i18n.rs` and the dialog text is filled at
    runtime.** `resources/app.rc` keeps the layout but its captions are empty;
    `settings/dialog.rs` fills them from the current table in `WM_INITDIALOG`, so the language
    can change without reopening the window. Changing the language is an `Effect::Language`:
    it calls `i18n::set_language`, `Tray::relabel` (menu `set_text`, never a new tray icon) and
    the dialog refills its combos. `logging::tests::source_strings_are_english` skips
    `i18n.rs`; `i18n::tests` keeps the English table free of CJK instead. `docs/SPIKE.md` §13.
24. **A path-registered tray icon is identified by `(exe path, uID)`.** The tooltip is stored in
    the key (`InitialTooltip`) but is not part of the identity, and the shell writes a
    `NotifyIconSettings` key only for identities it has not seen yet. `tray-icon` numbers icons
    from a per-process counter, so a failed GUID pass shifts every later uID (the fallback gets
    `N+1..2N`) and the user's pins land on other readouts or vanish. Never treat a missing key
    as a rejection (rule 17), and do not kill a running instance with `taskkill /F`: the dead
    identity stays in the Explorer session and no key is written for it again until Explorer
    restarts (a normal `Exit` or the installer's `WM_CLOSE` does not do this).
    `docs/SPIKE.md` §15.

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

- The `release` job downloads the artifacts from the `package` job, checks that the tag matches
  the binary's `VERSIONINFO`, and publishes `tray-monitor-<version>-windows-x64.zip`,
  `...-setup.exe`, `...msix` and `SHA256SUMS` (LF).
- The same tag hands the MSIX to the Store as a **draft** (`store-draft` in the pipeline) and stops
  there. This product is priced per market, so the submission API cannot carry the base price back
  and `--priceId` moves it off advanced pricing; the CLI refuses to publish either way. A human
  checks `Pricing and availability` in Partner Center, then commits with the **Store** workflow
  (`action=commit`) or `msstore submission publish`. See
  [docs/RELEASE-AUTOMATION.md](docs/RELEASE-AUTOMATION.md) §8-7.
- The installer and MSIX defaults are documented in [docs/PACKAGING.md](docs/PACKAGING.md).
  Store identity and signing are not configured yet; the MSIX is unsigned.
- Assume the archive is extracted in place: an update must not change the path of
  `tray-monitor.exe`, or every user loses their tray setting.

## Testing

- Pure logic (value formatting, delta arithmetic, layout selection, config parsing, the glyph
  table and glyph caching) is unit tested with `cargo test` and needs no display.
- The pixel font is deterministic, so icon rendering is tested with exact pixels plus invariants
  (glyph table shape, string widths, output stays 1-bit at every scale). The only GDI left is the
  preview DIB, where handle leaks are checked.
- The tray itself has no automated test. Use `make run`, then check the readouts, the context
  menu and `%LOCALAPPDATA%\tray-monitor\log.txt`. The log records a heartbeat every 60 updates
  with the current values and the glyph cache hit rate.
- Log messages are English only. `logging::tests::source_strings_are_english` rejects Hangul
  in any non-test string literal, because `{error}`/`{note}` values end up in the log too. The
  one exception is `src/i18n.rs`, whose English table is checked by `i18n::tests` instead.
  Comments and test assertions stay in Korean.
- The installer and MSIX need a desktop session for their end-to-end checks; CI covers the
  silent install/uninstall and the `makeappx` round trip.
