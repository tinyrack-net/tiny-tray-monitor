//! `App`을 스레드 지역에 두고 타이머 콜백과 메시지 루프가 함께 접근한다.
//!
//! 타이머는 `WndProc`에서 불리고 종료 판단은 메시지 루프에서 한다. 두 곳이 같은
//! `App`을 봐야 하므로 소유권을 옮기지 않고 빌려 쓴다.
//!
//! 모달 대화상자(`MessageBox`)는 자체 메시지 루프를 돌리므로 그 안에서 타이머가
//! 다시 들어올 수 있다. 그때는 이미 빌려진 상태이므로 조용히 건너뛴다.

use std::cell::RefCell;
use std::time::Duration;

use crate::app::App;

thread_local! {
    static APP: RefCell<Option<App>> = const { RefCell::new(None) };
}

/// 앱을 설치한다. 설치 전에는 모든 접근이 조용히 무시된다.
pub fn install(app: App) {
    APP.with(|cell| *cell.borrow_mut() = Some(app));
}

/// `App`을 빌려 쓴다. 쓰이는 중이면(재진입) `None`이다.
pub fn with<R>(f: impl FnOnce(&mut App) -> R) -> Option<R> {
    APP.with(|cell| match cell.try_borrow_mut() {
        Ok(mut slot) => slot.as_mut().map(f),
        Err(_) => None,
    })
}

/// 타이머 콜백.
pub fn tick() {
    with(|app| app.tick());
}

/// 트레이/메뉴 이벤트 처리.
pub fn drain_events() {
    with(|app| app.drain_events());
}

/// 설정 창 열기를 요청한다.
///
/// 메뉴 핸들러와 IPC 창은 여기까지만 한다. 실제로 창을 여는 것은 메시지 루프이고,
/// 그때는 `App`을 빌리고 있지 않다(모달 루프에서도 타이머가 계속 돈다).
pub fn request_settings() {
    with(|app| app.request_settings());
}

/// 종료를 요청한다. installer가 업그레이드 전에 IPC 창으로 보낸다.
pub fn request_exit() {
    with(|app| app.request_exit());
}

/// 요청이 있으면 가져가고 지운다.
pub fn take_settings_request() -> bool {
    with(|app| app.take_settings_request()).unwrap_or(false)
}

pub fn poll_interval() -> Duration {
    with(|app| app.poll_interval()).unwrap_or(Duration::from_secs(1))
}

/// 앱이 없으면 종료로 본다.
///
/// 단, 재진입으로 잠깐 빌릴 수 없는 상태는 종료가 아니다. 일시적인 대여 실패로
/// 앱이 죽으면 안 되므로 계속 실행으로 본다.
pub fn should_exit() -> bool {
    APP.with(|cell| match cell.try_borrow() {
        Ok(slot) => slot.as_ref().map(|app| app.should_exit()).unwrap_or(true),
        Err(_) => false,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_app_is_treated_as_exit() {
        // 테스트는 앱을 설치하지 않는다.
        assert!(should_exit());
        assert_eq!(poll_interval(), Duration::from_secs(1));
        tick();
        drain_events();
    }

    /// 재진입으로 잠깐 빌릴 수 없을 때 종료하면 안 된다.
    #[test]
    fn should_exit_is_false_when_the_app_is_borrowed() {
        APP.with(|cell| {
            let _guard = cell.borrow_mut();
            assert!(!should_exit(), "대여 중에는 종료가 아니다");
        });
    }
}
