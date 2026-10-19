//! 주기 갱신용 숨은 창.
//!
//! 트레이 메뉴(`TrackPopupMenu`)와 대화상자(`MessageBox`)는 자체 메시지 루프를 돌린다.
//! 그래서 타이머가 **재진입**할 수 있다.
//!
//! - 타이머를 스레드 큐로 받으면(`SetTimer(None, ...)`) 모달 루프 동안 `WM_TIMER`가
//!   버려져 아이콘이 멈춘다(실측).
//! - 숨은 창의 `WndProc`에서 받으면 멈추지 않지만, 콜백 실행 중에 콜백을 보관한
//!   `RefCell`을 계속 빌리고 있으면 재진입한 타이머가 패닉을 낸다(실측).
//!
//! 그래서 콜백을 **꺼내서 실행하고 다시 넣는다.** 실행 중 재진입한 타이머는 콜백이
//! 비어 있으므로 조용히 건너뛴다.

use std::cell::RefCell;

use windows::Win32::Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, HWND_MESSAGE, KillTimer, RegisterClassW,
    SetTimer, WINDOW_EX_STYLE, WINDOW_STYLE, WM_TIMER, WNDCLASSW,
};
use windows::core::PCWSTR;

thread_local! {
    static ON_TICK: RefCell<Option<Box<dyn FnMut()>>> = const { RefCell::new(None) };
}

const CLASS_NAME: &str = "tray-monitor-timer";

/// 콜백을 꺼내 실행하고 되돌려 놓는다. 재진입하면 콜백이 없어 건너뛴다.
fn dispatch_tick() {
    let callback = ON_TICK.with(|slot| slot.try_borrow_mut().ok().and_then(|mut slot| slot.take()));

    let Some(mut callback) = callback else {
        // 콜백이 없거나 다른 호출이 실행 중이다.
        return;
    };
    callback();

    ON_TICK.with(|slot| {
        if let Ok(mut slot) = slot.try_borrow_mut() {
            *slot = Some(callback);
        }
    });
}

unsafe extern "system" fn wnd_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if message == WM_TIMER {
        dispatch_tick();
        return LRESULT(0);
    }
    unsafe { DefWindowProcW(hwnd, message, wparam, lparam) }
}

pub struct TimerWindow {
    hwnd: HWND,
    timer: usize,
}

impl TimerWindow {
    pub fn new(interval_ms: u32, on_tick: impl FnMut() + 'static) -> Result<Self, String> {
        ON_TICK.with(|slot| *slot.borrow_mut() = Some(Box::new(on_tick)));

        let class_name = crate::win32::wide(CLASS_NAME);
        unsafe {
            let instance: HINSTANCE = GetModuleHandleW(None).map_err(|e| e.to_string())?.into();
            let class = WNDCLASSW {
                lpfnWndProc: Some(wnd_proc),
                hInstance: instance,
                lpszClassName: PCWSTR(class_name.as_ptr()),
                ..Default::default()
            };
            // 같은 클래스를 두 번 등록하면 실패하지만 그대로 진행해도 된다.
            RegisterClassW(&class);

            let hwnd = CreateWindowExW(
                WINDOW_EX_STYLE(0),
                PCWSTR(class_name.as_ptr()),
                PCWSTR::null(),
                WINDOW_STYLE(0),
                0,
                0,
                0,
                0,
                Some(HWND_MESSAGE),
                None,
                Some(instance),
                None,
            )
            .map_err(|e| format!("타이머 창을 만들 수 없다: {e}"))?;

            // hwnd를 주면 요청한 id가 그대로 쓰인다.
            let timer = SetTimer(Some(hwnd), 1, interval_ms, None);
            if timer == 0 {
                return Err("SetTimer 가 실패했다".to_string());
            }
            Ok(Self { hwnd, timer })
        }
    }

    /// 타이머 id. 진단용.
    pub fn id(&self) -> usize {
        self.timer
    }
}

impl Drop for TimerWindow {
    fn drop(&mut self) {
        ON_TICK.with(|slot| *slot.borrow_mut() = None);
        unsafe {
            let _ = KillTimer(Some(self.hwnd), self.timer);
            let _ = DestroyWindow(self.hwnd);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;
    use std::rc::Rc;

    use super::*;

    /// 테스트 간 간섭을 막기 위해 콜백을 비운다.
    fn clear() {
        ON_TICK.with(|slot| *slot.borrow_mut() = None);
    }

    #[test]
    fn callback_runs_on_each_dispatch() {
        clear();
        let count = Rc::new(Cell::new(0));
        let counter = count.clone();
        ON_TICK.with(|slot| {
            *slot.borrow_mut() = Some(Box::new(move || counter.set(counter.get() + 1)));
        });

        dispatch_tick();
        dispatch_tick();
        assert_eq!(count.get(), 2);
        clear();
    }

    /// 모달 대화상자 안에서 타이머가 재진입해도 패닉 없이 건너뛰어야 한다.
    #[test]
    fn reentrant_dispatch_is_skipped_without_panicking() {
        clear();
        let count = Rc::new(Cell::new(0));
        let outer = count.clone();
        let inner = count.clone();
        ON_TICK.with(|slot| {
            *slot.borrow_mut() = Some(Box::new(move || {
                outer.set(outer.get() + 1);
                inner.set(inner.get() + 1);
                if outer.get() == 1 {
                    // 콜백 실행 중 재진입
                    dispatch_tick();
                }
            }));
        });

        dispatch_tick();
        assert_eq!(
            count.get(),
            2,
            "바깥 콜백만 두 번 실행되고 재진입은 건너뛴다"
        );
        clear();
    }

    #[test]
    fn dispatch_without_callback_is_a_noop() {
        clear();
        dispatch_tick();
    }
}
