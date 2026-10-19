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
    let Some(expected) = command_line() else {
        return false;
    };
    CURRENT_USER
        .open(RUN_KEY)
        .and_then(|key| key.get_string(VALUE_NAME))
        .map(|value| value.eq_ignore_ascii_case(&expected))
        .unwrap_or(false)
}

/// 자동 실행 등록/해제.
pub fn set(enabled: bool) -> Result<(), String> {
    if enabled {
        let command = command_line().ok_or_else(|| "실행 경로를 알 수 없다".to_string())?;
        CURRENT_USER
            .create(RUN_KEY)
            .map_err(|e| e.to_string())?
            .set_string(VALUE_NAME, &command)
            .map_err(|e| e.to_string())
    } else {
        match CURRENT_USER.open(RUN_KEY) {
            Ok(key) => key.remove_value(VALUE_NAME).map_err(|e| e.to_string()),
            // 키 자체가 없으면 이미 해제된 상태다.
            Err(_) => Ok(()),
        }
    }
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
}
