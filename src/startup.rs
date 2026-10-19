//! Windows 시작 시 자동 실행 등록.
//!
//! `HKCU\Software\Microsoft\Windows\CurrentVersion\Run`에 값을 넣는다.
//! exe 경로가 바뀌면 값을 갱신해야 하므로, "등록되어 있는가"는 현재 경로와 비교해 판단한다.

use windows_registry::CURRENT_USER;

const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
const VALUE_NAME: &str = "tray-monitor";

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
    is_enabled_named(VALUE_NAME)
}

/// 자동 실행 등록/해제.
pub fn set(enabled: bool) -> Result<(), String> {
    set_named(VALUE_NAME, enabled)
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
        let command = command_line().ok_or_else(|| "실행 경로를 알 수 없다".to_string())?;
        return key.set_string(name, &command).map_err(|e| e.to_string());
    }

    // 이미 없으면 할 일이 없다.
    if key.get_string(name).is_err() {
        return Ok(());
    }
    key.remove_value(name).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paths_with_spaces_are_quoted() {
        assert_eq!(
            quote_command(r"C:\Program Files\tray-monitor\tray-monitor.exe"),
            r#""C:\Program Files\tray-monitor\tray-monitor.exe""#
        );
    }

    /// 실제 레지스트리를 건드리지 않고 읽기만 한다.
    #[test]
    fn is_enabled_is_readable() {
        let _ = is_enabled();
    }

    /// 시험용 이름으로 넣었다 뺄 수 있어야 한다.
    ///
    /// 해제 경로는 읽기 전용 키로 열어 `Access is denied`로 실패한 적이 있다.
    #[test]
    fn enabling_and_disabling_round_trips() {
        let name = format!("tray-monitor-test-{}", std::process::id());
        set_named(&name, true).expect("등록");
        assert!(is_enabled_named(&name));

        set_named(&name, false).expect("해제");
        assert!(!is_enabled_named(&name));

        // 이미 없는 값을 또 해제해도 성공으로본다.
        set_named(&name, false).expect("두 번째 해제");
    }
}
