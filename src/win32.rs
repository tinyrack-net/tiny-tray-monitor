//! Win32 공용 헬퍼. `unsafe`는 이 모듈과 [crate::icon::gdi]에만 둔다.

use windows::Win32::Foundation::{ERROR_ALREADY_EXISTS, GetLastError, HWND};
use windows::Win32::Storage::FileSystem::{MOVEFILE_REPLACE_EXISTING, MoveFileExW};
use windows::Win32::System::Threading::CreateMutexW;
use windows::Win32::UI::HiDpi::{
    DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, SetProcessDpiAwarenessContext,
};
use windows::Win32::UI::Shell::{
    NIF_GUID, NIF_TIP, NIM_MODIFY, NOTIFYICONDATAW, Shell_NotifyIconW, ShellExecuteW,
};
use windows::Win32::UI::WindowsAndMessaging::{
    GetSystemMetrics, IDYES, MB_ICONINFORMATION, MB_OK, MB_SETFOREGROUND, MB_TOPMOST, MB_YESNO,
    MESSAGEBOX_STYLE, MessageBoxW, SM_CXSMICON, SW_SHOWNORMAL,
};
use windows::core::{GUID, PCWSTR};

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

/// GUID로 등록된 트레이 아이콘의 툴팁을 바꾼다.
///
/// `tray-icon` 0.26.1의 `set_tooltip`은 `NIF_GUID` 없이 `NIM_MODIFY`를 보내 GUID
/// 아이콘에서는 항상 실패한다. 같은 호출을 직접 보내 우회한다. 반환값은 아이콘이
/// 실제로 등록됐는지 확인하는 탐침으로도 쓸 수 있다.
pub fn set_guid_tooltip(hwnd: *mut core::ffi::c_void, guid: u128, tooltip: &str) -> bool {
    let text = wide(tooltip);
    let mut data = NOTIFYICONDATAW {
        cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
        hWnd: HWND(hwnd),
        uFlags: NIF_TIP | NIF_GUID,
        guidItem: GUID::from_u128(guid),
        ..unsafe { std::mem::zeroed() }
    };
    for (slot, unit) in data.szTip.iter_mut().zip(&text) {
        *slot = *unit;
    }
    unsafe { Shell_NotifyIconW(NIM_MODIFY, &data) }.as_bool()
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

/// `from`을 `to`로 바꾼다. 대상이 있으면 대체한다.
///
/// 설정 파일을 임시 파일에 먼저 쓰고 이 함수로 교체한다. 저장 도중에 죽어도 반쯤 쓰인
/// 설정이 남아 다음 시작에 기본값으로 돌아가는 일이 없다.
pub fn replace_file(from: &std::path::Path, to: &std::path::Path) -> Result<(), String> {
    let from = wide(&from.display().to_string());
    let to = wide(&to.display().to_string());
    unsafe {
        MoveFileExW(
            PCWSTR(from.as_ptr()),
            PCWSTR(to.as_ptr()),
            MOVEFILE_REPLACE_EXISTING,
        )
    }
    .map_err(|e| e.to_string())
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

/// 대화상자와 그 안의 컨트롤.
///
/// 설정 창이 쓰는 Win32 호출을 여기에 모아 둔다(`unsafe`는 이 모듈과 [crate::icon::gdi]에만
/// 둔다는 경계). 바깥에는 안전한 Rust만 넘어간다.
pub mod dialog {
    use windows::Win32::Foundation::{HWND, LPARAM, POINT, RECT, WPARAM};
    use windows::Win32::Graphics::Gdi::{
        DeleteObject, GetMonitorInfoW, HBITMAP, HGDIOBJ, HMONITOR, InvalidateRect,
        MONITOR_DEFAULTTONEAREST, MONITORINFO, MonitorFromPoint,
    };
    use windows::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows::Win32::UI::Controls::{
        BST_CHECKED, ICC_STANDARD_CLASSES, INITCOMMONCONTROLSEX, InitCommonControlsEx,
    };
    use windows::Win32::UI::WindowsAndMessaging::{
        BM_GETCHECK, BM_SETCHECK, CB_ADDSTRING, CB_GETCURSEL, CB_SETCURSEL, DialogBoxParamW,
        EndDialog, GetCursorPos, GetDlgItemTextW, GetWindowRect, IMAGE_BITMAP, PostMessageW,
        STM_SETIMAGE, SWP_NOACTIVATE, SWP_NOSIZE, SWP_NOZORDER, SendDlgItemMessageW,
        SetDlgItemTextW, SetForegroundWindow, SetWindowPos,
    };
    use windows::core::PCWSTR;

    /// 표준 컨트롤 클래스를 등록한다.
    ///
    /// 비주얼 스타일은 매니페스트가 켜지만(comctl32 v6), 클래스 등록은 따로 알려 줘야 한다.
    pub fn init_common_controls() {
        let classes = INITCOMMONCONTROLSEX {
            dwSize: std::mem::size_of::<INITCOMMONCONTROLSEX>() as u32,
            dwICC: ICC_STANDARD_CLASSES,
        };
        let _ = unsafe { InitCommonControlsEx(&classes) };
    }

    /// 대화상자를 모달로 띄운다. 닫힐 때까지 돌아오지 않는다.
    pub fn show(id: u16, procedure: extern "system" fn(HWND, u32, WPARAM, LPARAM) -> isize) {
        let instance = unsafe { GetModuleHandleW(None) }.unwrap_or_default();
        // 리소스 id는 문자열이 아니라 정수 포인터로 넘긴다(MAKEINTRESOURCE).
        let name = PCWSTR(id as usize as *const u16);
        unsafe {
            DialogBoxParamW(
                Some(instance.into()),
                name,
                None,
                Some(procedure),
                LPARAM(0),
            )
        };
    }

    /// 대화상자를 닫는다.
    pub fn end(hwnd: HWND) {
        let _ = unsafe { EndDialog(hwnd, 0) };
    }

    /// 처리되지 않은 메시지를 창 큐에 넣는다. input-synchronous 핸들러에서
    /// WinRT 호출을 피할 때 쓴다.
    pub fn post(hwnd: HWND, message: u32) {
        let _ = unsafe { PostMessageW(Some(hwnd), message, WPARAM(0), LPARAM(0)) };
    }

    /// 체크박스·라디오의 체크 상태를 바꾼다.
    pub fn set_check(hwnd: HWND, id: i32, checked: bool) {
        let state = if checked { BST_CHECKED.0 } else { 0 };
        unsafe {
            SendDlgItemMessageW(hwnd, id, BM_SETCHECK, WPARAM(state as usize), LPARAM(0));
        }
    }

    /// 체크되어 있는가.
    pub fn is_check(hwnd: HWND, id: i32) -> bool {
        let state = unsafe { SendDlgItemMessageW(hwnd, id, BM_GETCHECK, WPARAM(0), LPARAM(0)) };
        state.0 == BST_CHECKED.0 as isize
    }

    /// 컨트롤의 글자를 바꾼다.
    pub fn set_text(hwnd: HWND, id: i32, text: &str) {
        let text = super::wide(text);
        let _ = unsafe { SetDlgItemTextW(hwnd, id, PCWSTR(text.as_ptr())) };
    }

    /// 컨트롤의 글자를 읽는다. 너무 길면 잘린다(입력 칸은 짧다).
    pub fn text(hwnd: HWND, id: i32) -> String {
        let mut buffer = [0u16; 64];
        let length = unsafe { GetDlgItemTextW(hwnd, id, &mut buffer) } as usize;
        String::from_utf16_lossy(&buffer[..length.min(buffer.len())])
    }

    /// 콤보에 항목을 넣고 그 위치를 돌려준다.
    pub fn combo_add(hwnd: HWND, id: i32, text: &str) -> i32 {
        let text = super::wide(text);
        let index = unsafe {
            SendDlgItemMessageW(
                hwnd,
                id,
                CB_ADDSTRING,
                WPARAM(0),
                LPARAM(text.as_ptr() as isize),
            )
        };
        index.0 as i32
    }

    /// 콤보에서 고른 위치. 고른 것이 없으면 -1.
    pub fn combo_selection(hwnd: HWND, id: i32) -> i32 {
        let index = unsafe { SendDlgItemMessageW(hwnd, id, CB_GETCURSEL, WPARAM(0), LPARAM(0)) };
        index.0 as i32
    }

    /// 콤보의 선택을 바꾼다. `CBN_SELCHANGE`는 오지 않는다.
    pub fn combo_select(hwnd: HWND, id: i32, index: i32) {
        unsafe {
            SendDlgItemMessageW(hwnd, id, CB_SETCURSEL, WPARAM(index as usize), LPARAM(0));
        }
    }

    /// 정적 컨트롤의 비트맵을 바꾸고 **이전 비트맵을 지운다.**
    ///
    /// `STM_SETIMAGE`는 이전 핸들을 돌려주므로 여기서 지우지 않으면 갱신할 때마다 GDI
    /// 핸들이 샌다.
    pub fn set_bitmap(hwnd: HWND, id: i32, bitmap: HBITMAP) {
        let previous = unsafe {
            SendDlgItemMessageW(
                hwnd,
                id,
                STM_SETIMAGE,
                WPARAM(IMAGE_BITMAP.0 as usize),
                LPARAM(bitmap.0 as isize),
            )
        };
        let previous = HBITMAP(previous.0 as *mut core::ffi::c_void);
        if !previous.is_invalid() {
            let _ = unsafe { DeleteObject(HGDIOBJ(previous.0)) };
        }
    }

    /// 커서가 있는 모니터의 작업 영역 가운데로 옮긴다.
    ///
    /// PMv2라 크기는 시스템이 배율에 맞춰 정한다. 위치만 잡아 준다.
    pub fn center_on_cursor_monitor(hwnd: HWND) {
        let mut cursor = POINT::default();
        if unsafe { GetCursorPos(&mut cursor) }.is_err() {
            return;
        }

        let monitor: HMONITOR = unsafe { MonitorFromPoint(cursor, MONITOR_DEFAULTTONEAREST) };
        let mut info = MONITORINFO {
            cbSize: std::mem::size_of::<MONITORINFO>() as u32,
            ..Default::default()
        };
        if !unsafe { GetMonitorInfoW(monitor, &mut info) }.as_bool() {
            return;
        }

        let mut rect = RECT::default();
        if unsafe { GetWindowRect(hwnd, &mut rect) }.is_err() {
            return;
        }

        let work = info.rcWork;
        let x = work.left + ((work.right - work.left) - (rect.right - rect.left)) / 2;
        let y = work.top + ((work.bottom - work.top) - (rect.bottom - rect.top)) / 2;
        let _ = unsafe {
            SetWindowPos(
                hwnd,
                None,
                x,
                y,
                0,
                0,
                SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE,
            )
        };
    }

    /// 창을 다시 칠하게 한다. 테마가 바뀌었을 때 쓴다.
    pub fn invalidate(hwnd: HWND) {
        let _ = unsafe { InvalidateRect(Some(hwnd), None, true) };
    }

    /// 대화상자를 앞으로 가져온다. 트레이 클릭으로 열면 이미 활성 상태다.
    pub fn bring_to_front(hwnd: HWND) {
        let _ = unsafe { SetForegroundWindow(hwnd) };
    }
}

/// 창 하나를 만들고 찾는다. 두 번째 실행을 넘겨받는 IPC 창이 쓴다.
pub mod window {
    use windows::Win32::Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, WPARAM};
    use windows::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows::Win32::System::Threading::GetCurrentProcessId;
    use windows::Win32::UI::WindowsAndMessaging::{
        CreateWindowExW, DefWindowProcW, DestroyWindow, EnumWindows, FindWindowW, GetClassNameW,
        GetWindowThreadProcessId, PostMessageW, RegisterClassW, WINDOW_EX_STYLE, WM_CLOSE,
        WNDCLASSW, WS_POPUP,
    };
    use windows::core::{BOOL, PCWSTR};

    /// 보이지 않는 창을 만든다.
    ///
    /// `HWND_MESSAGE`(메시지 전용 창)는 최상위 창이 아니라 `FindWindowW`로 찾히지 않으므로,
    /// 보여 주지 않는 일반 창을 만든다. `WS_POPUP` + `ShowWindow`를 부르지 않기 때문에
    /// 화면에 나타나지 않는다.
    pub fn create(
        class: &str,
        procedure: extern "system" fn(HWND, u32, WPARAM, LPARAM) -> LRESULT,
    ) -> Result<HWND, String> {
        let class_name = super::wide(class);
        unsafe {
            let instance: HINSTANCE = GetModuleHandleW(None).map_err(|e| e.to_string())?.into();
            let definition = WNDCLASSW {
                lpfnWndProc: Some(procedure),
                hInstance: instance,
                lpszClassName: PCWSTR(class_name.as_ptr()),
                ..Default::default()
            };
            // 같은 클래스를 두 번 등록하면 실패하지만 그대로 진행해도 된다.
            RegisterClassW(&definition);

            CreateWindowExW(
                WINDOW_EX_STYLE(0),
                PCWSTR(class_name.as_ptr()),
                PCWSTR::null(),
                WS_POPUP,
                0,
                0,
                0,
                0,
                None,
                None,
                Some(instance),
                None,
            )
            .map_err(|e| format!("{class} 창을 만들 수 없다: {e}"))
        }
    }

    /// 클래스 이름으로 창을 찾는다. 없으면 `None`.
    pub fn find(class: &str) -> Option<HWND> {
        let class = super::wide(class);
        unsafe { FindWindowW(PCWSTR(class.as_ptr()), PCWSTR::null()) }.ok()
    }

    /// 창에 메시지를 넣는다. 상대가 처리할 때까지 기다리지 않는다.
    pub fn notify(hwnd: HWND, message: u32) -> bool {
        unsafe { PostMessageW(Some(hwnd), message, WPARAM(0), LPARAM(0)).is_ok() }
    }

    /// 창을 없앤다.
    pub fn destroy(hwnd: HWND) {
        let _ = unsafe { DestroyWindow(hwnd) };
    }

    /// 이 프로세스의 최상위 대화상자(`#32770`)를 모두 닫는다.
    ///
    /// 종료 요청을 받았는데 설정 창이나 첫 실행 안내가 모달 루프를 돌리고 있으면,
    /// 메인 루프는 그 창이 닫힐 때까지 종료 검사를 하지 못한다.
    pub fn close_dialogs() {
        unsafe {
            let process = GetCurrentProcessId() as isize;
            let _ = EnumWindows(Some(close_dialog), LPARAM(process));
        }
    }

    extern "system" fn close_dialog(hwnd: HWND, lparam: LPARAM) -> BOOL {
        let process = lparam.0 as u32;
        let mut pid = 0u32;
        unsafe {
            GetWindowThreadProcessId(hwnd, Some(&mut pid));
        }
        if pid != process {
            return BOOL(1);
        }

        let mut class = [0u16; 32];
        let len = unsafe { GetClassNameW(hwnd, &mut class) };
        if len <= 0 || String::from_utf16_lossy(&class[..len as usize]) != "#32770" {
            return BOOL(1);
        }

        // 설정 창은 WM_CLOSE에서 다크 모드 자원을 정리하고, 첫 실행 안내/메시지
        // 상자도 WM_CLOSE로 닫힌다(실측).
        let _ = unsafe { PostMessageW(Some(hwnd), WM_CLOSE, WPARAM(0), LPARAM(0)) };
        BOOL(1)
    }

    /// 프로시저가 처리하지 않은 메시지를 기본 처리로 넘긴다.
    pub fn default_procedure(hwnd: HWND, message: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
        unsafe { DefWindowProcW(hwnd, message, wparam, lparam) }
    }
}

/// 설정 창과 트레이 메뉴의 다크 모드.
///
/// Windows에는 공개 API가 없다. `uxtheme.dll`의 **비공식 ordinal**을 쓴다(실측: Windows 11 26H2
/// build 26300에 133/135/136이 그대로 있다). 하나라도 없으면 아무것도 하지 않고 라이트로 남는다.
pub mod dark {
    use core::ffi::c_void;
    use std::cell::{Cell, RefCell};

    use windows::Win32::Foundation::{COLORREF, HWND, LPARAM, WPARAM};
    use windows::Win32::Graphics::Gdi::{
        CreateSolidBrush, DeleteObject, HBRUSH, HDC, HGDIOBJ, SetBkColor, SetBkMode, SetTextColor,
        TRANSPARENT,
    };
    use windows::Win32::System::LibraryLoader::{GetModuleHandleW, GetProcAddress, LoadLibraryW};
    use windows::Win32::UI::Controls::SetWindowTheme;
    use windows::Win32::UI::WindowsAndMessaging::{
        EnumChildWindows, WM_CTLCOLORBTN, WM_CTLCOLORDLG, WM_CTLCOLOREDIT, WM_CTLCOLORLISTBOX,
        WM_CTLCOLORSTATIC,
    };
    use windows::core::{BOOL, PCSTR, PCWSTR, w};

    /// `PreferredAppMode`. ForceDark면 앱의 메뉴까지 어두워진다.
    const FORCE_DARK: i32 = 2;
    const FORCE_LIGHT: i32 = 3;

    /// COLORREF는 `0x00BBGGRR`이다. 작업 표시줄 배경(32,32,32)과 같은 색을 쓴다.
    const BACKGROUND: u32 = 0x0020_2020;
    const TEXT: u32 = 0x00F0_F0F0;

    type SetPreferredAppMode = unsafe extern "system" fn(i32) -> i32;
    type AllowDarkModeForWindow = unsafe extern "system" fn(HWND, i32) -> i32;
    type FlushMenuThemes = unsafe extern "system" fn();

    thread_local! {
        /// 배경 브러시. 하나만 만들어 돌려 쓴다.
        static BRUSH: RefCell<Option<HBRUSH>> = const { RefCell::new(None) };
        /// 자식을 훑는 동안 쓸 함수. 열거 콜백은 인자를 하나밖에 못 받는다.
        static ALLOW: Cell<Option<AllowDarkModeForWindow>> = const { Cell::new(None) };
    }

    /// uxtheme의 비공식 ordinal을 함수 포인터로 바꾼다. 반환 타입은 부르는 쪽이 정한다.
    fn uxtheme<T: Copy>(ordinal: usize) -> Option<T> {
        let module = unsafe { GetModuleHandleW(w!("uxtheme.dll")) }
            .or_else(|_| unsafe { LoadLibraryW(w!("uxtheme.dll")) })
            .ok()?;
        // ordinal은 문자열이 아니라 정수 포인터로 넘긴다(MAKEINTRESOURCE).
        let address = unsafe { GetProcAddress(module, PCSTR(ordinal as *const u8)) }?;
        Some(unsafe { std::mem::transmute_copy::<_, T>(&address) })
    }

    /// 다크 모드에 필요한 함수가 모두 있는가. 진단용.
    pub fn available() -> bool {
        uxtheme::<AllowDarkModeForWindow>(133).is_some()
            && uxtheme::<SetPreferredAppMode>(135).is_some()
            && uxtheme::<FlushMenuThemes>(136).is_some()
    }

    /// 앱 전체를 지금 테마에 맞춘다.
    ///
    /// 메뉴는 창이 아니라 앱 단위로 칠해지므로 여기서 한 번 정한다(트레이 우클릭 메뉴 포함).
    pub fn set_app_mode(dark: bool) {
        let Some(set_mode) = uxtheme::<SetPreferredAppMode>(135) else {
            return;
        };
        unsafe { set_mode(if dark { FORCE_DARK } else { FORCE_LIGHT }) };

        // 이미 떠 있는 메뉴에도 반영한다.
        if let Some(flush) = uxtheme::<FlushMenuThemes>(136) {
            unsafe { flush() };
        }
    }

    /// 창과 그 자식 컨트롤에 테마를 입힌다.
    pub fn apply_to_window(hwnd: HWND, dark: bool) {
        let allow = uxtheme::<AllowDarkModeForWindow>(133);
        if let Some(allow) = allow {
            unsafe { allow(hwnd, i32::from(dark)) };
        }
        paint_theme(hwnd, dark);

        // 자식 컨트롤은 각자 테마를 갖는다. 하나씩 입혀 준다.
        ALLOW.with(|slot| slot.set(if dark { allow } else { None }));
        let _ = unsafe { EnumChildWindows(Some(hwnd), Some(theme_child), LPARAM(0)) };
        ALLOW.with(|slot| slot.set(None));

        if !dark {
            forget();
        }
    }

    /// 다크 모드를 그만 쓸 때 배경 브러시를 돌려준다.
    pub fn forget() {
        BRUSH.with(|cell| {
            if let Some(brush) = cell.borrow_mut().take() {
                let _ = unsafe { DeleteObject(HGDIOBJ(brush.0)) };
            }
        });
    }

    /// `WM_CTLCOLOR*` 처리. 다크일 때만 색을 바꾸고 배경 브러시를 돌려준다.
    ///
    /// `None`이면 시스템 기본 처리를 따른다.
    pub fn paint(dark: bool, message: u32, wparam: WPARAM) -> Option<isize> {
        if !dark || !is_color_message(message) {
            return None;
        }

        let device = HDC(wparam.0 as *mut c_void);
        let brush = brush();

        unsafe {
            SetTextColor(device, COLORREF(TEXT));
            SetBkColor(device, COLORREF(BACKGROUND));
            // 정적 컨트롤과 버튼은 배경을 비워야 대화상자 배경이 비친다.
            if message == WM_CTLCOLORSTATIC || message == WM_CTLCOLORBTN {
                SetBkMode(device, TRANSPARENT);
            }
        }

        Some(brush.0 as isize)
    }

    /// 이 메시지가 색을 돌려주는 메시지인가.
    pub fn is_color_message(message: u32) -> bool {
        matches!(
            message,
            WM_CTLCOLORDLG
                | WM_CTLCOLORSTATIC
                | WM_CTLCOLORBTN
                | WM_CTLCOLOREDIT
                | WM_CTLCOLORLISTBOX
        )
    }

    extern "system" fn theme_child(child: HWND, _: LPARAM) -> BOOL {
        ALLOW.with(|slot| {
            if let Some(allow) = slot.get() {
                unsafe { allow(child, 1) };
            }
        });
        paint_theme(child, true);
        true.into()
    }

    fn paint_theme(hwnd: HWND, dark: bool) {
        let application = if dark {
            w!("DarkMode_Explorer")
        } else {
            PCWSTR::null()
        };
        let _ = unsafe { SetWindowTheme(hwnd, application, PCWSTR::null()) };
    }

    /// 배경 브러시. 없으면 만든다.
    fn brush() -> HBRUSH {
        BRUSH.with(|cell| {
            let mut slot = cell.borrow_mut();
            *slot.get_or_insert_with(|| unsafe { CreateSolidBrush(COLORREF(BACKGROUND)) })
        })
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
