//! Win32 공용 헬퍼. `unsafe`는 이 모듈과 [crate::icon::gdi]에만 둔다.

use windows::Win32::Foundation::{ERROR_ALREADY_EXISTS, GetLastError};
use windows::Win32::System::Threading::CreateMutexW;
use windows::Win32::UI::HiDpi::{
    DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, SetProcessDpiAwarenessContext,
};
use windows::Win32::UI::Shell::ShellExecuteW;
use windows::Win32::UI::WindowsAndMessaging::{
    GetSystemMetrics, IDYES, MB_ICONINFORMATION, MB_OK, MB_SETFOREGROUND, MB_TOPMOST, MB_YESNO,
    MESSAGEBOX_STYLE, MessageBoxW, SM_CXSMICON, SW_SHOWNORMAL,
};
use windows::core::PCWSTR;

/// NUL 종료 UTF-16 문자열.
pub fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}

/// 작업 표시줄 DPI를 따라 아이콘을 그리도록 프로세스를 설정한다.
///
/// `resources/app.manifest`가 `PerMonitorV2`를 선언하면 이 호출은 "이미 설정됨"으로
/// 실패한다. 그 실패가 매니페스트가 살아 있다는 증거이므로 결과를 돌려준다.
///
/// 반환값: `true` = 이 호출이 설정했다(매니페스트가 없다), `false` = 이미 설정되어 있었다.
pub fn set_dpi_awareness() -> bool {
    unsafe { SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2).is_ok() }
}

/// 트레이 아이콘 한 칸의 크기(px). 배율에 따라 16/24/32 등이 된다.
pub fn tray_icon_size() -> i32 {
    unsafe { GetSystemMetrics(SM_CXSMICON) }.max(16)
}

/// 중복 실행을 막는다. 이미 실행 중이면 `false`.
///
/// 뮤텍스 핸들은 프로세스가 끝날 때까지 유지해야 하므로 의도적으로 해제하지 않는다.
pub fn acquire_single_instance() -> bool {
    acquire_named("Local\\tray-monitor-single-instance")
}

/// 이름을 지정해 중복 실행을 확인한다. 테스트가 실행 중인 앱과 충돌하지 않게 분리했다.
pub fn acquire_named(name: &str) -> bool {
    let name = wide(name);
    match unsafe { CreateMutexW(None, false, PCWSTR(name.as_ptr())) } {
        Ok(_) => (unsafe { GetLastError() }).0 != ERROR_ALREADY_EXISTS.0,
        // 확인하지 못하면 실행을 막지 않는다.
        Err(_) => true,
    }
}

/// 기본 브라우저나 설정 앱으로 URI를 연다.
pub fn open_url(url: &str) -> bool {
    let operation = wide("open");
    let target = wide(url);
    let result = unsafe {
        ShellExecuteW(
            None,
            PCWSTR(operation.as_ptr()),
            PCWSTR(target.as_ptr()),
            PCWSTR::null(),
            PCWSTR::null(),
            SW_SHOWNORMAL,
        )
    };
    result.0 as usize > 32
}

/// 확인/취소 대화상자. 예를 누르면 `true`.
///
/// 소유 창이 없어 다른 창 뒤에 깔리므로 항상 앞으로 가져온다.
pub fn confirm(title: &str, text: &str) -> bool {
    let title = wide(title);
    let text = wide(text);
    let style = MB_YESNO | MB_ICONINFORMATION | MB_SETFOREGROUND | MB_TOPMOST;
    let result = unsafe { MessageBoxW(None, PCWSTR(text.as_ptr()), PCWSTR(title.as_ptr()), style) };
    result == IDYES
}

/// 정보 대화상자. 확인/취소와 같은 이유로 앞으로 가져온다.
pub fn inform(title: &str, text: &str) {
    let title = wide(title);
    let text = wide(text);
    let style: MESSAGEBOX_STYLE = MB_OK | MB_ICONINFORMATION | MB_SETFOREGROUND | MB_TOPMOST;
    unsafe {
        MessageBoxW(None, PCWSTR(text.as_ptr()), PCWSTR(title.as_ptr()), style);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wide_is_nul_terminated() {
        let w = wide("CPU");
        assert_eq!(w.last(), Some(&0));
        assert_eq!(w.len(), 4);
    }

    #[test]
    fn tray_icon_size_is_sane() {
        let size = tray_icon_size();
        assert!((16..=256).contains(&size), "{size}");
    }

    #[test]
    fn second_instance_is_detected() {
        // 실행 중인 앱과 겹치지 않도록 테스트 전용 이름을 쓴다.
        let name = format!("Local\tray-monitor-test-{}", std::process::id());
        let first = acquire_named(&name);
        let second = acquire_named(&name);
        assert!(first, "첫 호출은 통과해야 한다");
        assert!(!second, "두 번째 호출은 중복으로 판정되어야 한다");
    }

    #[test]
    fn different_names_do_not_collide() {
        let a = format!("Local\tray-monitor-a-{}", std::process::id());
        let b = format!("Local\tray-monitor-b-{}", std::process::id());
        assert!(acquire_named(&a));
        assert!(acquire_named(&b));
    }
}
