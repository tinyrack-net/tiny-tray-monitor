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
                logging::warn(format!("{error} (두 번째 실행이 설정 창을 열 수 없다)"));
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
        // 메시지 루프 안이라 `App`을 빌리고 있지 않다. 실제로 창을 여는 것은 루프가 한다.
        runtime::request_settings();
        return LRESULT(0);
    }
    if message == WM_CLOSE {
        // installer가 파일을 덮어쓰기 전에 정상 종료를 요청한다.
        runtime::request_exit();
        return LRESULT(0);
    }
    win32::window::default_procedure(hwnd, message, wparam, lparam)
}

/// 실행 중인 인스턴스에 설정 창을 열어 달라고 한다. 전달했으면 `true`.
pub fn request_settings() -> bool {
    let Some(hwnd) = win32::window::find(CLASS_NAME) else {
        return false;
    };
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
