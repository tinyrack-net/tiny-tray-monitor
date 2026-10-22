//! Win32 공용 헬퍼. `unsafe`는 이 모듈과 [crate::icon::gdi]에만 둔다.

use windows::Win32::Foundation::{ERROR_ALREADY_EXISTS, GetLastError, HWND};
use windows::Win32::Storage::FileSystem::{MOVEFILE_REPLACE_EXISTING, MoveFileExW};
use windows::Win32::System::Threading::CreateMutexW;
use windows::Win32::UI::HiDpi::{
    DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, GetDpiForSystem, GetDpiForWindow,
    GetSystemMetricsForDpi, SetProcessDpiAwarenessContext,
};
use windows::Win32::UI::Shell::{
    NIF_GUID, NIF_TIP, NIM_MODIFY, NOTIFYICONDATAW, Shell_NotifyIconW, ShellExecuteW,
};
use windows::Win32::UI::WindowsAndMessaging::{
    FindWindowW, GetSystemMetrics, MB_ICONINFORMATION, MB_OK, MB_SETFOREGROUND, MB_TOPMOST,
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

/// 트레이 아이콘 한 칸의 크기(px). 배율에 따라 16/20/24/…/36 등이 된다.
///
/// `GetSystemMetrics(SM_CXSMICON)`은 쓰지 않는다. PerMonitorV2 프로세스에서는 그 값이
/// 프로세스가 시작할 때의 시스템 DPI에 묶여, 실행 중에 배율을 바꾸면 옛 크기의 아이콘을
/// 셸이 늘려 흐릿해진다(`docs/SPIKE.md` §16). 트레이를 실제로 그리는 주 작업 표시줄의
/// DPI로 계산하고, 그 창을 찾지 못하면 시스템 DPI로 물러선다.
///
/// 매 틱 부르므로 창 핸들을 캐시하지 않는다. Explorer가 다시 시작해도 따라간다.
pub fn tray_icon_size() -> i32 {
    let dpi = taskbar_dpi().unwrap_or_else(|| unsafe { GetDpiForSystem() });
    icon_size_for_dpi(dpi)
}

/// 주어진 DPI의 작은 아이콘 크기. 최소 16px.
pub fn icon_size_for_dpi(dpi: u32) -> i32 {
    unsafe { GetSystemMetricsForDpi(SM_CXSMICON, dpi) }.max(16)
}

/// 주 작업 표시줄(`Shell_TrayWnd`)의 DPI. 창이 없으면(Explorer 재시작 중 등) `None`.
pub fn taskbar_dpi() -> Option<u32> {
    let class = wide("Shell_TrayWnd");
    let hwnd = unsafe { FindWindowW(PCWSTR(class.as_ptr()), PCWSTR::null()) }.ok()?;
    match unsafe { GetDpiForWindow(hwnd) } {
        0 => None,
        dpi => Some(dpi),
    }
}

/// DPI 진단 값. `--dpi`가 출력한다.
pub struct DpiReport {
    /// `GetSystemMetrics(SM_CXSMICON)`: 예전 방식. 시작 시점 값에 묶이는지 본다.
    pub legacy_px: i32,
    /// `GetDpiForSystem`
    pub system_dpi: u32,
    /// `GetDpiForWindow(Shell_TrayWnd)`
    pub taskbar_dpi: Option<u32>,
    /// [tray_icon_size]가 고르는 크기.
    pub px: i32,
}

pub fn dpi_report() -> DpiReport {
    DpiReport {
        legacy_px: unsafe { GetSystemMetrics(SM_CXSMICON) },
        system_dpi: unsafe { GetDpiForSystem() },
        taskbar_dpi: taskbar_dpi(),
        px: tray_icon_size(),
    }
}

/// Windows 표시 언어의 LANGID. 지원하지 않는 언어는 호출자가 영어로 처리한다.
pub fn user_language_id() -> u16 {
    unsafe { windows::Win32::Globalization::GetUserDefaultUILanguage() }
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
    acquire_named("Local\\TinyTrayMonitor.SingleInstance")
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

/// 정보 대화상자. 소유 창이 없어 다른 창 뒤에 깔리므로 항상 앞으로 가져온다.
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
    use windows::Win32::Foundation::{COLORREF, HWND, LPARAM, POINT, RECT, WPARAM};
    use windows::Win32::Graphics::Gdi::{
        COLOR_BTNFACE, CreateFontIndirectW, CreateSolidBrush, DT_END_ELLIPSIS, DT_NOPREFIX,
        DT_SINGLELINE, DT_VCENTER, DeleteObject, DrawTextW, FW_SEMIBOLD, FillRect, GetMonitorInfoW,
        GetObjectW, GetSysColor, GetSysColorBrush, HDC, HFONT, HGDIOBJ, HMONITOR, InvalidateRect,
        LOGFONTW, MONITOR_DEFAULTTONEAREST, MONITORINFO, MonitorFromPoint, SelectObject, SetBkMode,
        SetTextColor, TRANSPARENT,
    };
    use windows::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows::Win32::UI::Controls::{
        BST_CHECKED, DRAWITEMSTRUCT, ICC_STANDARD_CLASSES, INITCOMMONCONTROLSEX,
        InitCommonControlsEx, MEASUREITEMSTRUCT, ODS_SELECTED,
    };
    use windows::Win32::UI::Input::KeyboardAndMouse::EnableWindow;
    use windows::Win32::UI::WindowsAndMessaging::{
        BM_GETCHECK, BM_SETCHECK, CB_ADDSTRING, CB_GETCURSEL, CB_RESETCONTENT, CB_SETCURSEL,
        DialogBoxParamW, EndDialog, GetCursorPos, GetDlgItem, GetDlgItemTextW, GetWindowRect,
        LB_ADDSTRING, LB_GETCURSEL, LB_RESETCONTENT, LB_SETCURSEL, MapDialogRect, PostMessageW,
        SW_HIDE, SW_SHOW, SWP_NOACTIVATE, SWP_NOSIZE, SWP_NOZORDER, SendDlgItemMessageW,
        SendMessageW, SetDlgItemTextW, SetForegroundWindow, SetWindowPos, SetWindowTextW,
        ShowWindow, WM_GETFONT, WM_SETFONT,
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

    /// 대화상자의 제목을 바꾼다.
    pub fn set_window_text(hwnd: HWND, text: &str) {
        let text = super::wide(text);
        let _ = unsafe { SetWindowTextW(hwnd, PCWSTR(text.as_ptr())) };
    }

    /// 콤보의 항목을 전부 지운다. 언어를 바꾸면 항목을 다시 넣어야 한다.
    pub fn combo_reset(hwnd: HWND, id: i32) {
        unsafe {
            SendDlgItemMessageW(hwnd, id, CB_RESETCONTENT, WPARAM(0), LPARAM(0));
        }
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

    /// `lparam`(`WM_CTLCOLOR*`의 컨트롤 핸들)이 이 id의 컨트롤인가.
    pub fn is_control(hwnd: HWND, id: i32, lparam: LPARAM) -> bool {
        unsafe { GetDlgItem(Some(hwnd), id) }.is_ok_and(|control| control.0 as isize == lparam.0)
    }

    /// 밝은 테마의 대화상자 배경색(R, G, B).
    pub fn face_color() -> (u8, u8, u8) {
        let value = unsafe { GetSysColor(COLOR_BTNFACE) };
        (
            (value & 0xFF) as u8,
            ((value >> 8) & 0xFF) as u8,
            ((value >> 16) & 0xFF) as u8,
        )
    }

    /// 밝은 테마의 대화상자 배경 브러시. 시스템 브러시라 지우지 않는다.
    pub fn face_brush() -> isize {
        unsafe { GetSysColorBrush(COLOR_BTNFACE) }.0 as isize
    }

    /// 컨트롤을 보이거나 숨긴다. 페이지를 바꿀 때 쓴다.
    pub fn show_control(hwnd: HWND, id: i32, visible: bool) {
        if let Ok(control) = unsafe { GetDlgItem(Some(hwnd), id) } {
            let _ = unsafe { ShowWindow(control, if visible { SW_SHOW } else { SW_HIDE }) };
        }
    }

    /// 컨트롤을 켜거나 끈다(회색).
    pub fn enable_control(hwnd: HWND, id: i32, enabled: bool) {
        if let Ok(control) = unsafe { GetDlgItem(Some(hwnd), id) } {
            let _ = unsafe { EnableWindow(control, enabled) };
        }
    }

    /// 컨트롤을 대화상자 단위(DLU) 좌표로 옮기고 크기를 바꾼다.
    ///
    /// 페이지마다 장치 그룹의 높이가 달라 `.rc` 하나로는 배치를 정할 수 없다. DLU로 계산해야
    /// 글꼴·배율이 바뀌어도 `.rc`와 같은 비율을 유지한다.
    pub fn place_control(hwnd: HWND, id: i32, x: i32, y: i32, width: i32, height: i32) {
        let Ok(control) = (unsafe { GetDlgItem(Some(hwnd), id) }) else {
            return;
        };
        let mut rect = RECT {
            left: x,
            top: y,
            right: x + width,
            bottom: y + height,
        };
        if unsafe { MapDialogRect(hwnd, &mut rect) }.is_err() {
            return;
        }
        let _ = unsafe {
            SetWindowPos(
                control,
                None,
                rect.left,
                rect.top,
                rect.right - rect.left,
                rect.bottom - rect.top,
                SWP_NOZORDER | SWP_NOACTIVATE,
            )
        };
    }

    /// 대화상자 단위를 픽셀로. (가로, 세로)
    pub fn dialog_units(hwnd: HWND, x: i32, y: i32) -> (i32, i32) {
        let mut rect = RECT {
            left: 0,
            top: 0,
            right: x,
            bottom: y,
        };
        let _ = unsafe { MapDialogRect(hwnd, &mut rect) };
        (rect.right, rect.bottom)
    }

    /// 목록의 항목을 지우고 다시 넣는다.
    pub fn list_reset(hwnd: HWND, id: i32, items: &[&str]) {
        unsafe {
            SendDlgItemMessageW(hwnd, id, LB_RESETCONTENT, WPARAM(0), LPARAM(0));
        }
        for item in items {
            let text = super::wide(item);
            unsafe {
                SendDlgItemMessageW(
                    hwnd,
                    id,
                    LB_ADDSTRING,
                    WPARAM(0),
                    LPARAM(text.as_ptr() as isize),
                );
            }
        }
    }

    /// 목록에서 고른 위치. 고른 것이 없으면 -1.
    pub fn list_selection(hwnd: HWND, id: i32) -> i32 {
        let index = unsafe { SendDlgItemMessageW(hwnd, id, LB_GETCURSEL, WPARAM(0), LPARAM(0)) };
        index.0 as i32
    }

    /// 목록의 선택을 바꾼다. `LBN_SELCHANGE`는 오지 않는다.
    pub fn list_select(hwnd: HWND, id: i32, index: i32) {
        unsafe {
            SendDlgItemMessageW(hwnd, id, LB_SETCURSEL, WPARAM(index as usize), LPARAM(0));
        }
    }

    /// 대화상자 글꼴을 바탕으로 크기와 굵기를 바꾼 글꼴. 돌려받은 쪽이 [delete_font]로 지운다.
    ///
    /// `scale`은 백분율이다(150 = 1.5배). 대화상자 글꼴을 따라가므로 배율이 바뀌어도 맞는다.
    pub fn derived_font(hwnd: HWND, scale: i32, semibold: bool) -> Option<HFONT> {
        let base = unsafe { SendMessageW(hwnd, WM_GETFONT, Some(WPARAM(0)), Some(LPARAM(0))) };
        if base.0 == 0 {
            return None;
        }
        let mut font = LOGFONTW::default();
        let size = std::mem::size_of::<LOGFONTW>() as i32;
        let read = unsafe {
            GetObjectW(
                HGDIOBJ(base.0 as *mut core::ffi::c_void),
                size,
                Some(&mut font as *mut LOGFONTW as *mut core::ffi::c_void),
            )
        };
        if read == 0 {
            return None;
        }
        font.lfHeight = font.lfHeight * scale / 100;
        if semibold {
            font.lfWeight = FW_SEMIBOLD.0 as i32;
        }
        let created = unsafe { CreateFontIndirectW(&font) };
        (!created.is_invalid()).then_some(created)
    }

    /// 컨트롤 글꼴을 바꾼다. 글꼴은 부른 쪽이 들고 있다가 창이 닫힐 때 지운다.
    pub fn set_font(hwnd: HWND, id: i32, font: HFONT) {
        unsafe {
            SendDlgItemMessageW(hwnd, id, WM_SETFONT, WPARAM(font.0 as usize), LPARAM(1));
        }
    }

    pub fn delete_font(font: HFONT) {
        let _ = unsafe { DeleteObject(HGDIOBJ(font.0)) };
    }

    /// 오너드로 목록 항목 하나를 그리는 데 필요한 값.
    pub struct NavItem<'a> {
        pub text: &'a str,
        /// 왼쪽 막대와 점의 색(R, G, B).
        pub accent: (u8, u8, u8),
        pub background: (u8, u8, u8),
        pub selected_background: (u8, u8, u8),
        pub text_color: (u8, u8, u8),
    }

    /// `WM_MEASUREITEM`: 목록 항목 높이를 대화상자 단위로 정한다.
    ///
    /// # Safety
    /// `lparam`은 `WM_MEASUREITEM`이 넘긴 `MEASUREITEMSTRUCT` 포인터여야 한다.
    pub unsafe fn measure_item(hwnd: HWND, lparam: LPARAM, height_dlu: i32) {
        let item = lparam.0 as *mut MEASUREITEMSTRUCT;
        if let Some(item) = unsafe { item.as_mut() } {
            let (_, height) = dialog_units(hwnd, 0, height_dlu);
            item.itemHeight = height.max(1) as u32;
        }
    }

    /// `WM_DRAWITEM`이 그릴 항목. 목록의 몇 번째인지와 선택 여부.
    ///
    /// # Safety
    /// `lparam`은 `WM_DRAWITEM`이 넘긴 `DRAWITEMSTRUCT` 포인터여야 한다.
    pub unsafe fn draw_target(lparam: LPARAM) -> Option<(i32, usize, bool)> {
        let item = unsafe { (lparam.0 as *const DRAWITEMSTRUCT).as_ref() }?;
        let index = usize::try_from(item.itemID as i32).ok()?;
        Some((
            item.CtlID as i32,
            index,
            item.itemState.0 & ODS_SELECTED.0 != 0,
        ))
    }

    fn colorref((r, g, b): (u8, u8, u8)) -> COLORREF {
        COLORREF(r as u32 | (g as u32) << 8 | (b as u32) << 16)
    }

    fn fill(device: HDC, rect: &RECT, color: (u8, u8, u8)) {
        let brush = unsafe { CreateSolidBrush(colorref(color)) };
        unsafe { FillRect(device, rect, brush) };
        let _ = unsafe { DeleteObject(HGDIOBJ(brush.0)) };
    }

    /// 왼쪽 목록 항목을 그린다. 선택된 항목은 배경을 칠하고 왼쪽에 지표 색 막대를 둔다.
    ///
    /// # Safety
    /// `lparam`은 `WM_DRAWITEM`이 넘긴 `DRAWITEMSTRUCT` 포인터여야 한다.
    pub unsafe fn draw_nav_item(hwnd: HWND, lparam: LPARAM, item: &NavItem, selected: bool) {
        let Some(draw) = (unsafe { (lparam.0 as *const DRAWITEMSTRUCT).as_ref() }) else {
            return;
        };
        let device = draw.hDC;
        let rect = draw.rcItem;
        let (unit_x, unit_y) = dialog_units(hwnd, 4, 8);
        let unit_x = unit_x.max(1);

        fill(
            device,
            &rect,
            if selected {
                item.selected_background
            } else {
                item.background
            },
        );

        // 선택 막대: 왼쪽 가장자리, 위아래 여백을 둔 세로 막대.
        let bar_inset = (rect.bottom - rect.top) / 4;
        if selected {
            let bar = RECT {
                left: rect.left,
                top: rect.top + bar_inset,
                right: rect.left + (unit_x * 3 / 4).max(3),
                bottom: rect.bottom - bar_inset,
            };
            fill(device, &bar, item.accent);
        }

        // 지표 색 점.
        let dot = (unit_y / 2).max(6);
        let dot_left = rect.left + unit_x * 2;
        let dot_top = rect.top + (rect.bottom - rect.top - dot) / 2;
        fill(
            device,
            &RECT {
                left: dot_left,
                top: dot_top,
                right: dot_left + dot,
                bottom: dot_top + dot,
            },
            item.accent,
        );

        let mut text_rect = RECT {
            left: dot_left + dot + unit_x * 2,
            top: rect.top,
            right: rect.right - unit_x,
            bottom: rect.bottom,
        };
        let font =
            unsafe { SendMessageW(draw.hwndItem, WM_GETFONT, Some(WPARAM(0)), Some(LPARAM(0))) };
        let previous = (font.0 != 0)
            .then(|| unsafe { SelectObject(device, HGDIOBJ(font.0 as *mut core::ffi::c_void)) });
        unsafe {
            SetBkMode(device, TRANSPARENT);
            SetTextColor(device, colorref(item.text_color));
        }
        // 빈 문자열을 넘기지 않는다(AGENTS 규칙 3).
        if !item.text.is_empty() {
            let mut text: Vec<u16> = item.text.encode_utf16().collect();
            unsafe {
                DrawTextW(
                    device,
                    &mut text,
                    &mut text_rect,
                    DT_SINGLELINE | DT_VCENTER | DT_NOPREFIX | DT_END_ELLIPSIS,
                );
            }
        }
        if let Some(previous) = previous {
            unsafe { SelectObject(device, previous) };
        }
    }
}

/// 창 하나를 만들고 찾는다. 두 번째 실행을 넘겨받는 IPC 창이 쓴다.
pub mod window {
    use windows::Win32::Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, WPARAM};
    use windows::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows::Win32::System::Threading::GetCurrentProcessId;
    use windows::Win32::UI::WindowsAndMessaging::{
        AllowSetForegroundWindow, CreateWindowExW, DefWindowProcW, DestroyWindow, EnumWindows,
        FindWindowW, GetClassNameW, GetWindowThreadProcessId, PostMessageW, RegisterClassW,
        WINDOW_EX_STYLE, WM_CLOSE, WNDCLASSW, WS_POPUP,
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
            .map_err(|e| format!("cannot create the {class} window: {e}"))
        }
    }

    /// 클래스 이름으로 창을 찾는다. 없으면 `None`.
    pub fn find(class: &str) -> Option<HWND> {
        let class = super::wide(class);
        unsafe { FindWindowW(PCWSTR(class.as_ptr()), PCWSTR::null()) }.ok()
    }

    /// 이 창을 가진 프로세스가 창을 앞으로 가져올 수 있게 허락한다.
    ///
    /// 포그라운드 잠금 때문에 백그라운드 프로세스의 `SetForegroundWindow`는 거부된다. 사용자가
    /// 막 실행한 두 번째 프로세스가 포그라운드이므로, 메시지를 보내기 전에 권한을 넘긴다.
    pub fn allow_foreground(hwnd: HWND) {
        let mut process = 0u32;
        unsafe {
            GetWindowThreadProcessId(hwnd, Some(&mut process));
            if process != 0 {
                let _ = AllowSetForegroundWindow(process);
            }
        }
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
    use windows::Win32::Graphics::Dwm::{DWMWA_USE_IMMERSIVE_DARK_MODE, DwmSetWindowAttribute};
    use windows::Win32::Graphics::Gdi::{
        CreateSolidBrush, DeleteObject, HBRUSH, HDC, HGDIOBJ, SetBkColor, SetBkMode, SetTextColor,
        TRANSPARENT,
    };
    use windows::Win32::System::LibraryLoader::{GetModuleHandleW, GetProcAddress, LoadLibraryW};
    use windows::Win32::UI::Controls::{
        BP_RADIOBUTTON, CloseThemeData, GetThemeColor, OpenThemeData, RBS_UNCHECKEDNORMAL,
        SetWindowTheme, TMT_TEXTCOLOR,
    };
    use windows::Win32::UI::WindowsAndMessaging::{
        BS_AUTORADIOBUTTON, BS_GROUPBOX, BS_RADIOBUTTON, BS_TYPEMASK, EnumChildWindows, GWL_STYLE,
        GetClassNameW, GetWindowLongW, WM_CTLCOLORBTN, WM_CTLCOLORDLG, WM_CTLCOLOREDIT,
        WM_CTLCOLORLISTBOX, WM_CTLCOLORSTATIC,
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
        /// 자식을 훑는 동안 입힐 테마가 다크인가. [ALLOW]와 같은 이유로 둔다.
        static DARK: Cell<bool> = const { Cell::new(false) };
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
        // 제목 표시줄은 DWM이 그린다. 공개 API(Windows 10 20H1 이상)라 실패해도 무시한다.
        let value = windows::core::BOOL::from(dark);
        let _ = unsafe {
            DwmSetWindowAttribute(
                hwnd,
                DWMWA_USE_IMMERSIVE_DARK_MODE,
                &value as *const _ as *const c_void,
                std::mem::size_of::<windows::core::BOOL>() as u32,
            )
        };
        let allow = uxtheme::<AllowDarkModeForWindow>(133);
        if let Some(allow) = allow {
            unsafe { allow(hwnd, i32::from(dark)) };
        }
        paint_theme(hwnd, dark);

        // 자식 컨트롤은 각자 테마를 갖는다. 하나씩 입혀 준다.
        ALLOW.with(|slot| slot.set(if dark { allow } else { None }));
        DARK.with(|slot| slot.set(dark));
        let _ = unsafe { EnumChildWindows(Some(hwnd), Some(theme_child), LPARAM(0)) };
        ALLOW.with(|slot| slot.set(None));
        DARK.with(|slot| slot.set(false));

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

    /// 정적 컨트롤 하나의 글자색만 바꾼다(페이지 제목). 배경은 테마를 따른다.
    ///
    /// 다크면 다크 배경 브러시를, 아니면 `None`을 돌려줘 부른 쪽이 시스템 브러시를 쓰게 한다.
    pub fn paint_text(wparam: WPARAM, (r, g, b): (u8, u8, u8), dark: bool) -> Option<isize> {
        let device = HDC(wparam.0 as *mut c_void);
        let color = r as u32 | (g as u32) << 8 | (b as u32) << 16;
        unsafe {
            SetTextColor(device, COLORREF(color));
            SetBkMode(device, TRANSPARENT);
        }
        dark.then(|| brush().0 as isize)
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
        let dark = DARK.with(Cell::get);
        ALLOW.with(|slot| {
            if let Some(allow) = slot.get() {
                unsafe { allow(child, 1) };
            }
        });
        if dark && is_radio(child) {
            paint_radio(child);
        } else if dark && is_group_box(child) {
            paint_group_box(child);
        } else {
            paint_theme(child, dark);
        }
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

    /// `Button` 클래스 컨트롤의 종류(`BS_*`). 버튼이 아니면 `None`.
    fn button_kind(hwnd: HWND) -> Option<i32> {
        let mut class = [0u16; 16];
        let length = unsafe { GetClassNameW(hwnd, &mut class) };
        let is_button = usize::try_from(length)
            .ok()
            .and_then(|length| class.get(..length))
            .is_some_and(|name| String::from_utf16_lossy(name).eq_ignore_ascii_case("Button"));
        if !is_button {
            return None;
        }
        Some(unsafe { GetWindowLongW(hwnd, GWL_STYLE) } & BS_TYPEMASK)
    }

    /// 라디오 버튼인가. 라디오만 다크 테마가 따로 필요하다([paint_radio]).
    fn is_radio(hwnd: HWND) -> bool {
        matches!(button_kind(hwnd), Some(BS_RADIOBUTTON | BS_AUTORADIOBUTTON))
    }

    /// 그룹 박스인가. 그룹도 라디오처럼 다크 테마가 글자색을 정의하지 않는다([paint_group_box]).
    fn is_group_box(hwnd: HWND) -> bool {
        matches!(button_kind(hwnd), Some(BS_GROUPBOX))
    }

    /// 라디오 버튼에 다크 테마를 입힌다.
    ///
    /// `DarkMode_Explorer::Button`은 체크박스 글자색(흰색)만 정의하고 라디오 글자색은
    /// 정의하지 않아, 라디오 글자가 어두운 배경 위에 검정으로 그려진다(build 26300 실측).
    /// 라디오 글자색이 있는 `DarkMode_DarkTheme`을 먼저 쓰고, 그것도 없는 빌드에서는
    /// 비주얼 스타일을 꺼서 `WM_CTLCOLORSTATIC`의 글자색([TEXT])이 적용되게 한다.
    fn paint_radio(hwnd: HWND) {
        for application in [w!("DarkMode_DarkTheme"), w!("DarkMode_Explorer")] {
            if radio_text_is_themed(application) {
                let _ = unsafe { SetWindowTheme(hwnd, application, PCWSTR::null()) };
                return;
            }
        }
        // 빈 문자열은 이 컨트롤의 비주얼 스타일을 끈다.
        let _ = unsafe { SetWindowTheme(hwnd, w!(""), w!("")) };
    }

    /// 그룹 박스의 비주얼 스타일을 끈다.
    ///
    /// `DarkMode_Explorer::Button`은 그룹 박스 캡션의 글자색을 정의하지 않아, 캡션이 검정으로
    /// 그려져 어두운 배경에서 보이지 않는다(build 26300 실측). 스타일을 끄면 클래식 그룹
    /// 박스가 `WM_CTLCOLORSTATIC`의 글자색([TEXT])으로 캡션을 그린다. 프레임도 클래식으로
    /// 그려지지만 어두운 테마보다 대비가 오히려 낫다.
    fn paint_group_box(hwnd: HWND) {
        // 빈 문자열은 이 컨트롤의 비주얼 스타일을 끈다.
        let _ = unsafe { SetWindowTheme(hwnd, w!(""), w!("")) };
    }

    /// `<application>::Button` 테마가 라디오 글자색을 정의하는가.
    fn radio_text_is_themed(application: PCWSTR) -> bool {
        let name = format!("{}::Button", unsafe { application.display() });
        let wide: Vec<u16> = name.encode_utf16().chain(std::iter::once(0)).collect();
        let theme = unsafe { OpenThemeData(None, PCWSTR(wide.as_ptr())) };
        if theme.is_invalid() {
            return false;
        }
        let color = unsafe {
            GetThemeColor(
                theme,
                BP_RADIOBUTTON.0,
                RBS_UNCHECKEDNORMAL.0,
                TMT_TEXTCOLOR,
            )
        };
        let _ = unsafe { CloseThemeData(theme) };
        color.is_ok()
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

    /// 배율별 크기는 현재 프로세스 DPI와 무관하게 DPI로만 정해져야 한다.
    #[test]
    fn icon_size_follows_the_dpi() {
        for (dpi, px) in [
            (96, 16),
            (120, 20),
            (144, 24),
            (168, 28),
            (192, 32),
            (216, 36),
        ] {
            assert_eq!(icon_size_for_dpi(dpi), px, "{dpi} dpi");
        }
    }

    #[test]
    fn second_instance_is_detected() {
        // 실행 중인 앱과 겹치지 않도록 테스트 전용 이름을 쓴다.
        let name = format!("Local\\TinyTrayMonitor.Test.{}", std::process::id());
        let first = acquire_named(&name);
        let second = acquire_named(&name);
        assert!(first, "첫 호출은 통과해야 한다");
        assert!(!second, "두 번째 호출은 중복으로 판정되어야 한다");
    }

    #[test]
    fn different_names_do_not_collide() {
        let a = format!("Local\\TinyTrayMonitor.TestA.{}", std::process::id());
        let b = format!("Local\\TinyTrayMonitor.TestB.{}", std::process::id());
        assert!(acquire_named(&a));
        assert!(acquire_named(&b));
    }
}
