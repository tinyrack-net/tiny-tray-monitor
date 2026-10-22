//! Windows 시작 시 자동 실행 등록.
//!
//! 비패키지 빌드는 `HKCU\Software\Microsoft\Windows\CurrentVersion\Run`에 값을 넣는다.
//! MSIX 패키지 빌드에서는 레지스트리 쓰기가 가상화되어 실제 `Run` 키에 남지 않으므로
//! `StartupTask` API를 쓴다(실측은 docs/PACKAGING.md).

use std::time::{Duration, Instant};

use windows::ApplicationModel::StartupTask;
use windows::core::{HSTRING, Interface, RuntimeType};
use windows_future::{AsyncStatus, IAsyncInfo, IAsyncOperation};
use windows_registry::CURRENT_USER;

use crate::platform;

const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
const VALUE_NAME: &str = "TinyTrayMonitor";
/// v0.11.0 이전의 `Run` 값 이름. [`migrate_legacy`]가 새 이름으로 옮긴다.
const LEGACY_VALUE_NAME: &str = "tray-monitor";

/// AppxManifest.xml의 `uap5:StartupTask TaskId`와 같아야 한다.
const TASK_ID: &str = "TrayMonitorStartup";
/// 비동기 작업을 기다리는 최대 시간. 넘으면 실패로 보고 UI를 막지 않는다.
const TASK_TIMEOUT: Duration = Duration::from_secs(5);

/// 경로에 공백이 있으면 인용해야 한다.
pub fn quote_command(path: &str) -> String {
    format!("\"{path}\"")
}

fn command_line() -> Option<String> {
    let exe = std::env::current_exe().ok()?;
    Some(quote_command(&exe.display().to_string()))
}

/// 현재 실행 파일로 자동 실행이 등록되어 있는지.
pub fn is_enabled() -> bool {
    if platform::packaged() {
        return get_task()
            .and_then(|task| task.State().map_err(|e| e.to_string()))
            .map(|state| enabled_state(state.0))
            .unwrap_or(false);
    }
    is_enabled_named(VALUE_NAME)
}

/// 자동 실행 등록/해제.
pub fn set(enabled: bool) -> Result<(), String> {
    if platform::packaged() {
        return set_task(enabled);
    }
    set_named(VALUE_NAME, enabled)
}

/// v0.11.0 이전 `Run` 값(`tray-monitor`)을 지금 실행 파일의 새 값으로 옮긴다.
///
/// 압축본을 같은 폴더에 풀어 업데이트하면 옛 `tray-monitor.exe`가 남고, 옛 값이 그것을
/// 가리켜 로그인할 때 옛 버전이 뜬다. 옛 값이 있었다는 것은 사용자가 자동 실행을 켰다는
/// 뜻이므로 지금 실행 파일로 다시 등록한다. 한 일을 로그에 남길 문장으로 돌려준다.
pub fn migrate_legacy() -> Option<String> {
    if platform::packaged() {
        return None;
    }
    let key = CURRENT_USER.create(RUN_KEY).ok()?;
    let old = key.get_string(LEGACY_VALUE_NAME).ok()?;
    if let Err(error) = set_named(VALUE_NAME, true) {
        return Some(format!("could not move the startup entry ({old}): {error}"));
    }
    match key.remove_value(LEGACY_VALUE_NAME) {
        Ok(()) => Some(format!(
            "moved the startup entry from {LEGACY_VALUE_NAME} ({old}) to {VALUE_NAME}"
        )),
        Err(error) => Some(format!(
            "registered {VALUE_NAME} but could not remove {LEGACY_VALUE_NAME}: {error}"
        )),
    }
}

fn is_enabled_named(name: &str) -> bool {
    let Some(expected) = command_line() else {
        return false;
    };
    CURRENT_USER
        .open(RUN_KEY)
        .and_then(|key| key.get_string(name))
        .map(|value| value.eq_ignore_ascii_case(&expected))
        .unwrap_or(false)
}

/// `open`은 읽기 전용 키를 준다. 값을 지우려면 `create`로 쓰기 권한을 얻어야 한다.
///
/// (읽기 전용 키에서 `remove_value`를 부르면 `Access is denied`로 실패한다 — 실측.)
fn set_named(name: &str, enabled: bool) -> Result<(), String> {
    let key = CURRENT_USER.create(RUN_KEY).map_err(|e| e.to_string())?;

    if enabled {
        let command =
            command_line().ok_or_else(|| "cannot determine the executable path".to_string())?;
        return key.set_string(name, &command).map_err(|e| e.to_string());
    }

    // 이미 없으면 할 일이 없다.
    if key.get_string(name).is_err() {
        return Ok(());
    }
    key.remove_value(name).map_err(|e| e.to_string())
}

/// `StartupTaskState`의 값. `EnabledByPolicy`도 켜진 상태로 본다.
fn enabled_state(state: i32) -> bool {
    matches!(state, 2 | 4)
}

fn get_task() -> Result<StartupTask, String> {
    let operation = StartupTask::GetAsync(&HSTRING::from(TASK_ID))
        .map_err(|e| format!("cannot open the startup task: {e}"))?;
    wait(&operation)
}

fn set_task(enabled: bool) -> Result<(), String> {
    let task = get_task()?;
    if !enabled {
        return task.Disable().map_err(|e| e.to_string());
    }

    let operation = task.RequestEnableAsync().map_err(|e| e.to_string())?;
    let state = wait(&operation)?;
    if enabled_state(state.0) {
        return Ok(());
    }

    Err(match state.0 {
        1 => "Windows Settings has disabled startup for TinyTrayMonitor. \
              Turn it on in Settings > Apps > Startup."
            .to_string(),
        3 => "Startup is disabled by policy.".to_string(),
        _ => "Windows did not enable startup.".to_string(),
    })
}

/// `IAsyncOperation`을 폴링해 결과를 기다린다.
///
/// `windows-future`의 `Async::join`은 공개 API가 아니고 `get()`도 없어서,
/// `IAsyncInfo::Status()`를 직접 확인한다(SPIKE §11).
fn wait<T: RuntimeType + 'static>(operation: &IAsyncOperation<T>) -> Result<T, String> {
    let info: IAsyncInfo = operation.cast().map_err(|e| e.to_string())?;
    let deadline = Instant::now() + TASK_TIMEOUT;

    loop {
        let status = info.Status().map_err(|e| e.to_string())?;
        match status {
            AsyncStatus::Completed => return operation.GetResults().map_err(|e| e.to_string()),
            AsyncStatus::Canceled => return Err("the startup request was cancelled".to_string()),
            AsyncStatus::Error => {
                let code = info.ErrorCode().map(|code| code.0).unwrap_or(0);
                return Err(format!("the startup request failed (0x{code:08X})"));
            }
            _ => {}
        }

        if Instant::now() >= deadline {
            return Err("the startup request timed out".to_string());
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paths_with_spaces_are_quoted() {
        assert_eq!(
            quote_command(r"C:\Program Files\TinyTrayMonitor\tiny-tray-monitor.exe"),
            r#""C:\Program Files\TinyTrayMonitor\tiny-tray-monitor.exe""#
        );
    }

    /// 실제 레지스트리를 건드리지 않고 읽기만 한다.
    #[test]
    fn is_enabled_is_readable() {
        let _ = is_enabled();
    }

    /// 정책으로 켜진 상태도 켜짐이다.
    #[test]
    fn startup_state_mapping() {
        assert!(!enabled_state(0), "Disabled");
        assert!(!enabled_state(1), "DisabledByUser");
        assert!(enabled_state(2), "Enabled");
        assert!(!enabled_state(3), "DisabledByPolicy");
        assert!(enabled_state(4), "EnabledByPolicy");
    }

    /// 시험용 이름으로 넣었다 뺄 수 있어야 한다.
    ///
    /// 해제 경로는 읽기 전용 키로 열어 `Access is denied`로 실패한 적이 있다.
    #[test]
    fn enabling_and_disabling_round_trips() {
        let name = format!("tiny-tray-monitor-test-{}", std::process::id());
        set_named(&name, true).expect("등록");
        assert!(is_enabled_named(&name));

        set_named(&name, false).expect("해제");
        assert!(!is_enabled_named(&name));

        // 이미 없는 값을 또 해제해도 성공으로본다.
        set_named(&name, false).expect("두 번째 해제");
    }
}
