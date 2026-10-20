//! 이미 실행 중인 인스턴스에 설정 창을 열어 달라고 부탁한다.
//!
//! 트레이 메뉴에서만 설정을 열 수 있으면 exe를 다시 실행하는 사용자를 놓친다. 보이지 않는
//! 창 하나를 띄워 두고, 두 번째 프로세스가 그 창을 찾아 메시지를 보낸다.

use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};

use crate::logging;
use crate::runtime;
use crate::win32;

/// 창 클래스 이름. 두 프로세스가 같은 값을 봐야 한다.
const CLASS_NAME: &str = "tray-monitor-ipc";

/// 두 번째 실행이 보내는 메시지. `WM_APP` 영역이라 다른 메시지와 겹치지 않는다.
const WM_APP_OPEN_SETTINGS: u32 = 0x8000 + 1;

/// installer가 업그레이드 전에 보내는 종료 요청. 표준 `WM_CLOSE`를 그대로 쓴다.
const WM_CLOSE: u32 = 0x0010;

/// 이 인스턴스가 설정 창 요청을 받는 창.
pub struct Listener {
    hwnd: HWND,
}

impl Listener {
    /// 창을 만든다. 실패해도 앱은 계속 돌아야 하므로 `None`을 돌려준다.
    pub fn start() -> Option<Self> {
        match win32::window::create(CLASS_NAME, procedure) {
            Ok(hwnd) => Some(Self { hwnd }),
            Err(error) => {
                logging::warn(format!(
                    "{error} (a second launch cannot open the settings window)"
                ));
                None
            }
        }
    }
}

impl Drop for Listener {
    fn drop(&mut self) {
        win32::window::destroy(self.hwnd);
    }
}

extern "system" fn procedure(hwnd: HWND, message: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if message == WM_APP_OPEN_SETTINGS {
        // 열려 있으면 그 창을 앞으로 가져오고, 닫혀 있으면 메시지 루프가 연다.
        runtime::request_settings();
        return LRESULT(0);
    }
    if message == WM_CLOSE {
        // installer가 파일을 덮어쓰기 전에 정상 종료를 요청한다. 설정 창이나 첫 실행
        // 안내가 모달 루프를 돌리고 있으면 그것부터 닫아야 메인 루프가 종료를 본다.
        runtime::request_exit();
        win32::window::close_dialogs();
        return LRESULT(0);
    }
    win32::window::default_procedure(hwnd, message, wparam, lparam)
}

/// 실행 중인 인스턴스에 설정 창을 열어 달라고 한다. 전달했으면 `true`.
pub fn request_settings() -> bool {
    let Some(hwnd) = win32::window::find(CLASS_NAME) else {
        return false;
    };
    // 실행 중인 인스턴스는 백그라운드라 스스로는 창을 앞으로 가져오지 못한다.
    win32::window::allow_foreground(hwnd);
    win32::window::notify(hwnd, WM_APP_OPEN_SETTINGS)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// installer는 이 창에 `WM_CLOSE`를 보내 앱을 종료시킨다.
    #[test]
    fn the_window_accepts_the_close_message() {
        let listener = Listener::start().expect("IPC 창을 만들 수 있다");
        let hwnd = win32::window::find(CLASS_NAME).expect("클래스 이름으로 찾을 수 있다");
        assert!(win32::window::notify(hwnd, WM_CLOSE));
        drop(listener);
    }

    /// 두 번째 프로세스는 이 창을 찾아 메시지를 넣을 수 있어야 한다.
    #[test]
    fn the_window_can_be_found_and_notified() {
        let listener = Listener::start().expect("IPC 창을 만들 수 있다");
        let hwnd = win32::window::find(CLASS_NAME).expect("클래스 이름으로 찾을 수 있다");
        assert!(win32::window::notify(hwnd, WM_APP_OPEN_SETTINGS));
        drop(listener);
    }
}
