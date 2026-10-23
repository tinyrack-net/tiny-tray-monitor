//! 설정 대화상자.
//!
//! `resources/app.rc`의 `IDD_SETTINGS` 템플릿을 띄운다. 왼쪽 목록에서 페이지([Page])를 고르면
//! 오른쪽에 그 페이지의 컨트롤만 보인다. 값은 저장된 설정에서 읽어 오고, 컨트롤이 바뀔 때마다
//! 바뀐 항목만 적용한다 — 저장 버튼이 없다(즉시 적용).
//!
//! 분야 페이지(CPU·메모리·GPU·네트워크·디스크)는 같은 컨트롤을 돌려 쓴다: 표시 항목 체크박스
//! 두 개와 장치 콤보 하나. 어느 지표와 장치를 보여 줄지는 [super::pages]가 정한다.
//!
//! 아이콘 미리보기는 두지 않는다. 즉시 적용이라 트레이 아이콘이 곧 미리보기다.
//!
//! 템플릿의 글자는 전부 비어 있고, `WM_INITDIALOG`에서 [crate::i18n]의 표로 채운다. 그래서
//! 창을 닫지 않고 언어를 바꿀 수 있다.
//!
//! Win32 호출은 [crate::win32::dialog]에 모아 두었다. 이 모듈은 배치와 규칙만 다룬다.

use std::cell::{Cell, RefCell};

use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows::Win32::Graphics::Gdi::HFONT;
use windows::Win32::UI::WindowsAndMessaging::{
    CBN_SELCHANGE, EN_KILLFOCUS, LBN_SELCHANGE, WM_CLOSE, WM_COMMAND, WM_CTLCOLORLISTBOX,
    WM_CTLCOLORSTATIC, WM_DRAWITEM, WM_INITDIALOG, WM_MEASUREITEM, WM_SETTINGCHANGE,
    WM_THEMECHANGED,
};

use crate::config::{ColorMode, Config, UiTheme};
use crate::i18n::{self, Lang, Language};
use crate::icon::{self, Metric};
use crate::logging;
use crate::metrics::{self, format};
use crate::runtime;
use crate::settings::model::{self, SettingsDraft};
use crate::settings::pages::{self, Choices, DeviceKind, Page};
use crate::theme;
use crate::win32;

/// 리소스의 대화상자 id.
pub const IDD_SETTINGS: u16 = 101;

// 컨트롤 id. `resources/app.rc`의 `#define`과 같아야 한다(테스트로 대조한다).
pub const IDC_SHOW_1: i32 = 1001;
pub const IDC_SHOW_2: i32 = 1002;
pub const IDC_NAV: i32 = 1003;
pub const IDC_PAGE_TITLE: i32 = 1004;
pub const IDC_INTERVAL: i32 = 1006;
pub const IDC_INTERVAL_EDIT: i32 = 1007;
pub const IDC_INTERVAL_UNIT: i32 = 1008;
pub const IDC_COLOR_METRIC: i32 = 1009;
pub const IDC_COLOR_THEME: i32 = 1010;
pub const IDC_AUTOSTART: i32 = 1012;
pub const IDC_STATUS: i32 = 1013;
pub const IDC_VERSION: i32 = 1014;
pub const IDC_PAGE_HINT: i32 = 1015;
/// 목록과 페이지 사이의 세로 선. 코드는 건드리지 않고 `.rc` 대조에만 쓴다.
#[cfg(test)]
const IDC_SEPARATOR: i32 = 1016;
pub const IDC_RESET: i32 = 1017;
pub const IDC_GROUP_READOUTS: i32 = 1018;
pub const IDC_HINT_READOUTS: i32 = 1019;
pub const IDC_GROUP_UPDATE: i32 = 1020;
pub const IDC_HINT_UPDATE: i32 = 1021;
pub const IDC_GROUP_APPEARANCE: i32 = 1022;
pub const IDC_HINT_APPEARANCE: i32 = 1023;
pub const IDC_GROUP_STARTUP: i32 = 1024;
pub const IDC_HINT_STARTUP: i32 = 1025;
pub const IDC_LABEL_INTERVAL: i32 = 1026;
pub const IDC_LABEL_LANGUAGE: i32 = 1027;
pub const IDC_LANGUAGE: i32 = 1028;
pub const IDC_HINT_LANGUAGE: i32 = 1030;
pub const IDC_HINT_TASKBAR: i32 = 1031;
pub const IDC_OPEN_TASKBAR: i32 = 1032;
pub const IDC_GROUP_TRAY: i32 = 1033;
pub const IDC_GROUP_DEVICE: i32 = 1034;
pub const IDC_HINT_DEVICE: i32 = 1035;
pub const IDC_LABEL_DEVICE: i32 = 1036;
pub const IDC_DEVICE: i32 = 1037;
pub const IDC_DEVICE_INFO: i32 = 1038;
pub const IDC_LABEL_ICON_COLOR: i32 = 1039;
pub const IDC_LABEL_WINDOW_THEME: i32 = 1040;
pub const IDC_THEME_AUTO: i32 = 1041;
pub const IDC_THEME_LIGHT: i32 = 1042;
pub const IDC_THEME_DARK: i32 = 1043;

/// 창 테마 라디오. 순서는 [UiTheme::ALL]과 같다.
const THEME_CONTROLS: [(i32, UiTheme); 3] = [
    (IDC_THEME_AUTO, UiTheme::Auto),
    (IDC_THEME_LIGHT, UiTheme::Light),
    (IDC_THEME_DARK, UiTheme::Dark),
];

/// 닫기 버튼과 Esc가 보내는 id.
pub const IDC_CLOSE: i32 = 2;

/// "작업 표시줄 설정 열기" 버튼이 여는 Windows 설정 페이지.
const TASKBAR_SETTINGS_URI: &str = "ms-settings:taskbar";

/// `General` 페이지에만 보이는 컨트롤.
const GENERAL_CONTROLS: [i32; 24] = [
    IDC_GROUP_UPDATE,
    IDC_HINT_UPDATE,
    IDC_LABEL_INTERVAL,
    IDC_INTERVAL,
    IDC_INTERVAL_EDIT,
    IDC_INTERVAL_UNIT,
    IDC_GROUP_APPEARANCE,
    IDC_HINT_APPEARANCE,
    IDC_LABEL_ICON_COLOR,
    IDC_COLOR_METRIC,
    IDC_COLOR_THEME,
    IDC_LABEL_WINDOW_THEME,
    IDC_THEME_AUTO,
    IDC_THEME_LIGHT,
    IDC_THEME_DARK,
    IDC_GROUP_STARTUP,
    IDC_HINT_STARTUP,
    IDC_AUTOSTART,
    IDC_LABEL_LANGUAGE,
    IDC_LANGUAGE,
    IDC_HINT_LANGUAGE,
    IDC_GROUP_TRAY,
    IDC_HINT_TASKBAR,
    IDC_OPEN_TASKBAR,
];

/// 분야 페이지에 보이는 컨트롤. 일부는 페이지에 따라 다시 숨긴다([show_page]).
const METRIC_CONTROLS: [i32; 9] = [
    IDC_GROUP_READOUTS,
    IDC_HINT_READOUTS,
    IDC_SHOW_1,
    IDC_SHOW_2,
    IDC_GROUP_DEVICE,
    IDC_HINT_DEVICE,
    IDC_LABEL_DEVICE,
    IDC_DEVICE,
    IDC_DEVICE_INFO,
];

/// 표시 항목 체크박스. 페이지의 지표 순서와 같다.
const SHOW_CONTROLS: [i32; 2] = [IDC_SHOW_1, IDC_SHOW_2];

/// 목록 항목의 높이(대화상자 단위).
const NAV_ITEM_HEIGHT: i32 = 20;

/// 창이 열려 있는 동안의 상태.
#[derive(Default)]
struct State {
    page: Option<Page>,
    /// 지금 장치 콤보에 들어 있는 항목. 고른 위치를 값으로 바꿀 때 쓴다.
    choices: Option<Choices>,
    title_font: Option<HFONT>,
}

thread_local! {
    /// 테마를 입히는 중인가.
    ///
    /// `SetPreferredAppMode`와 `SetWindowTheme`은 다시 테마 메시지를 보낸다. 그 메시지를
    /// 그대로 처리하면 끝없이 되돌아 스택이 넘친다(실측: build 26300에서 스택 오버플로).
    static APPLYING: Cell<bool> = const { Cell::new(false) };
    /// 열려 있는 설정 창. 트레이 메뉴가 바꾼 값을 창에 알릴 때 쓴다.
    static WINDOW: Cell<Option<HWND>> = const { Cell::new(None) };
    /// 콤보 항목을 마지막으로 채운 언어. 언어가 바뀔 때만 다시 채운다.
    static RENDERED: Cell<Option<Lang>> = const { Cell::new(None) };
    /// 지금 페이지, 장치 콤보 항목, 제목 글꼴.
    static STATE: RefCell<State> = RefCell::new(State::default());
}

/// 자동 시작 토글을 미룰 때 쓰는 창 메시지. `WM_APP` 영역.
const WM_APP_AUTOSTART: u32 = 0x8000 + 2;

/// 바깥(트레이 메뉴)에서 값이 바뀌었으니 컨트롤을 다시 맞추라는 창 메시지.
const WM_APP_REFRESH: u32 = 0x8000 + 3;

/// 설정 창이 열려 있으면 앞으로 가져온다. 열려 있었으면 `true`.
///
/// 열려 있는 동안 들어온 "설정 열기"는 새 창을 예약하지 않고 이 함수로 끝낸다. 예약하면
/// 지금 창을 닫는 순간 메시지 루프가 그 요청을 보고 창을 한 번 더 띄운다(실측).
pub fn focus() -> bool {
    match WINDOW.with(Cell::get) {
        Some(hwnd) => {
            logging::info("settings window already open, bringing it to the front");
            win32::dialog::bring_to_front(hwnd);
            true
        }
        None => false,
    }
}

/// 설정 창이 열려 있으면 컨트롤을 지금 값에 다시 맞추게 한다.
///
/// 부르는 쪽은 `App`을 빌린 채이므로 여기서 바로 맞추지 않고 메시지로 미룬다.
pub fn refresh() {
    if let Some(hwnd) = WINDOW.with(Cell::get) {
        win32::dialog::post(hwnd, WM_APP_REFRESH);
    }
}

/// `Custom…`은 프리셋 뒤에 붙는다.
fn custom_index() -> i32 {
    model::INTERVAL_PRESETS.len() as i32
}

/// 대화상자를 띄운다. 닫힐 때까지 돌아오지 않는다.
///
/// **`App`을 빌린 채 부르면 안 된다.** 모달 루프 동안 타이머 틱이 전부 버려진다.
pub fn open() {
    if focus() {
        return;
    }
    win32::dialog::init_common_controls();
    logging::info("settings window opened");
    win32::dialog::show(IDD_SETTINGS, procedure);
    logging::info("settings window closed");
}

extern "system" fn procedure(hwnd: HWND, message: u32, wparam: WPARAM, lparam: LPARAM) -> isize {
    // 페이지 제목은 지표 색으로 칠한다. 나머지 색 메시지는 다크 모드일 때만 가로챈다.
    if message == WM_CTLCOLORSTATIC && win32::dialog::is_control(hwnd, IDC_PAGE_TITLE, lparam) {
        return paint_title(wparam);
    }
    // 목록의 빈 자리는 창 배경과 같은 색이어야 한다(밝은 테마에서 흰 칸이 남는다).
    if message == WM_CTLCOLORLISTBOX && !dark() && win32::dialog::is_control(hwnd, IDC_NAV, lparam)
    {
        return win32::dialog::face_brush();
    }
    if win32::dark::is_color_message(message) {
        return win32::dark::paint(dark(), message, wparam).unwrap_or(0);
    }

    match message {
        WM_INITDIALOG => {
            on_init(hwnd);
            1
        }
        WM_MEASUREITEM => {
            unsafe { win32::dialog::measure_item(hwnd, lparam, NAV_ITEM_HEIGHT) };
            1
        }
        WM_DRAWITEM => {
            draw_nav(hwnd, lparam);
            1
        }
        WM_COMMAND => {
            on_command(hwnd, wparam);
            1
        }
        WM_APP_AUTOSTART => {
            commit(hwnd, |draft| {
                draft.autostart = !draft.autostart;
                Ok(())
            });
            1
        }
        WM_APP_REFRESH => {
            if let Some(draft) = current() {
                sync_controls(hwnd, &draft);
            }
            1
        }
        // 작업 표시줄 테마가 바뀌면 창과 미리보기를 다시 맞춘다.
        WM_THEMECHANGED | WM_SETTINGCHANGE => {
            apply_theme(hwnd);
            1
        }
        WM_CLOSE => {
            finish(hwnd);
            1
        }
        _ => 0,
    }
}

/// 창을 어둡게 그리는가. 설정의 창 테마를 따르고, 자동이면 작업 표시줄을 따른다.
fn dark() -> bool {
    theme::ui_dark()
}

fn page() -> Page {
    STATE.with(|state| state.borrow().page.unwrap_or(Page::General))
}

/// 지표의 대표 색. `General`은 테마 전경색이다. 창 배경과 대비가 맞는 팔레트에서 고른다.
fn accent(page: Page) -> icon::Color {
    let palette = theme::ui_palette();
    match page.swatch() {
        Some(metric) => palette.value(metric),
        None => palette.mono,
    }
}

/// 창 배경색. 다크면 작업 표시줄 배경과 같은 색이다(`win32::dark`).
fn background() -> (u8, u8, u8) {
    if dark() {
        (32, 32, 32)
    } else {
        win32::dialog::face_color()
    }
}

/// 페이지 제목의 색. 지표 색을 쓰되 배경과 대비가 맞는 팔레트에서 고른다.
fn paint_title(wparam: WPARAM) -> isize {
    let color = accent(page());
    win32::dark::paint_text(wparam, (color.0, color.1, color.2), dark())
        .unwrap_or_else(win32::dialog::face_brush)
}

/// 왼쪽 목록 항목을 그린다.
fn draw_nav(hwnd: HWND, lparam: LPARAM) {
    let Some((control, index, selected)) = (unsafe { win32::dialog::draw_target(lparam) }) else {
        return;
    };
    if control != IDC_NAV {
        return;
    }
    let Some(page) = Page::from_index(index) else {
        return;
    };
    let dark = dark();
    let swatch = accent(page);
    let background = background();
    // 선택 배경은 배경에서 한 단계 떨어진 회색이다. Windows 11 설정 앱의 목록과 같은 방식.
    let selected_background = if dark { (55, 55, 55) } else { (225, 225, 225) };
    let text_color = if dark { (240, 240, 240) } else { (20, 20, 20) };
    let item = win32::dialog::NavItem {
        text: page.title(),
        accent: (swatch.0, swatch.1, swatch.2),
        background,
        selected_background,
        text_color,
    };
    unsafe { win32::dialog::draw_nav_item(hwnd, lparam, &item, selected) };
}

/// 창을 닫는다. 다크 모드용 GDI 자원과 글꼴도 함께 돌려준다.
fn finish(hwnd: HWND) {
    WINDOW.with(|slot| slot.set(None));
    RENDERED.with(|slot| slot.set(None));
    STATE.with(|state| {
        let mut state = state.borrow_mut();
        if let Some(font) = state.title_font.take() {
            win32::dialog::delete_font(font);
        }
        *state = State::default();
    });
    win32::dark::forget();
    win32::dialog::end(hwnd);
}

/// 테마를 맞춘다. 이 과정에서 다시 들어오는 테마 메시지는 무시한다.
///
/// `SetPreferredAppMode`와 `SetWindowTheme`이 테마 메시지를 다시 보내므로, 가드가 없으면
/// 재진입이 끝없이 쌓여 스택이 넘친다.
fn apply_theme(hwnd: HWND) {
    if APPLYING.with(|flag| flag.replace(true)) {
        return;
    }

    let dark = dark();
    win32::dark::set_app_mode(dark);
    win32::dark::apply_to_window(hwnd, dark);
    win32::dialog::invalidate(hwnd);

    APPLYING.with(|flag| flag.set(false));
}

fn on_init(hwnd: HWND) {
    WINDOW.with(|slot| slot.set(Some(hwnd)));
    win32::dialog::center_on_cursor_monitor(hwnd);
    win32::dialog::bring_to_front(hwnd);

    // 제목은 대화상자 글꼴을 1.5배, 반굵게 키운다.
    if let Some(font) = win32::dialog::derived_font(hwnd, 150, true) {
        win32::dialog::set_font(hwnd, IDC_PAGE_TITLE, font);
        STATE.with(|state| state.borrow_mut().title_font = Some(font));
    }

    apply_theme(hwnd);

    win32::dialog::set_text(
        hwnd,
        IDC_VERSION,
        &format!("v{}", env!("CARGO_PKG_VERSION")),
    );

    let draft = current().unwrap_or_else(|| SettingsDraft::from_config(&Config::default(), false));
    sync_controls(hwnd, &draft);
    show_page(hwnd, Page::General, &draft);
}

fn on_command(hwnd: HWND, wparam: WPARAM) {
    let id = (wparam.0 & 0xFFFF) as i32;
    let notification = ((wparam.0 >> 16) & 0xFFFF) as u32;

    if let Some(slot) = SHOW_CONTROLS.iter().position(|control| *control == id) {
        if let Some(metric) = page().metrics().get(slot) {
            toggle_metric(hwnd, id, *metric);
        }
        return;
    }

    match id {
        IDC_CLOSE => finish(hwnd),
        IDC_RESET => reset(hwnd),
        IDC_OPEN_TASKBAR => open_taskbar_settings(),
        IDC_NAV if notification == LBN_SELCHANGE => page_selected(hwnd),
        IDC_DEVICE if notification == CBN_SELCHANGE => device_selected(hwnd),
        IDC_INTERVAL if notification == CBN_SELCHANGE => interval_selected(hwnd),
        IDC_INTERVAL_EDIT if notification == EN_KILLFOCUS => interval_typed(hwnd),
        IDC_LANGUAGE if notification == CBN_SELCHANGE => language_selected(hwnd),
        IDC_COLOR_METRIC => commit(hwnd, |draft| {
            draft.color_mode = ColorMode::Metric;
            Ok(())
        }),
        IDC_COLOR_THEME => commit(hwnd, |draft| {
            draft.color_mode = ColorMode::Theme;
            Ok(())
        }),
        IDC_THEME_AUTO | IDC_THEME_LIGHT | IDC_THEME_DARK => theme_selected(hwnd, id),
        IDC_AUTOSTART => {
            // WM_COMMAND은 input-synchronous 호출이다. 여기서 StartupTask의
            // RequestEnableAsync를 부르면 RPC_E_CANTCALLOUT_ININPUTSYNCCALL로
            // 실패하므로(실측), 같은 스레드의 다음 메시지로 미룬다.
            win32::dialog::post(hwnd, WM_APP_AUTOSTART);
        }
        _ => {}
    }
}

/// 지금 적용되어 있는 값.
fn current() -> Option<SettingsDraft> {
    runtime::with(|app| app.draft())
}

/// 바뀐 항목을 적용하고 화면을 실제 값에 다시 맞춘다.
///
/// `change`가 거절하면(마지막 지표를 끄려 할 때) 아무것도 적용하지 않고 화면을 되돌린다.
fn commit(hwnd: HWND, change: impl FnOnce(&mut SettingsDraft) -> Result<(), String>) {
    let Some(applied) = current() else {
        return;
    };

    let mut next = applied.clone();
    if let Err(reason) = change(&mut next) {
        sync_controls(hwnd, &applied);
        set_status(hwnd, &reason);
        return;
    }

    let mut messages: Vec<String> = next.validate().iter().map(note_text).collect();
    let effects = model::diff(&applied, &next);
    // 반영 실패는 알림 창이 아니라 상태줄로 알린다. 알림 창은 중첩 루프를 돌리고,
    // 그 동안 `App` 대여가 풀리지 않아 트레이 갱신이 멈춘다(실측).
    messages.extend(runtime::with(|app| app.apply_effects(&effects)).unwrap_or_default());

    sync_controls(hwnd, &next);
    set_status(hwnd, &messages.join(" · "));
}

/// 값 보정 메모를 지금 언어의 문구로 바꾼다.
fn note_text(note: &model::Note) -> String {
    match note {
        model::Note::IntervalClamped { from, to } => i18n::note_interval_clamped(*from, *to),
        model::Note::NoReadoutEnabled => i18n::note_no_readout(),
    }
}

/// 컨트롤의 글자를 지금 언어로 채운다.
///
/// 콤보 항목은 언어가 바뀔 때만 다시 채운다. 다시 채우면 선택이 풀리므로 매번 하지 않는다.
fn apply_text(hwnd: HWND, draft: &SettingsDraft) {
    let text = i18n::t();
    win32::dialog::set_window_text(hwnd, text.window_title);
    win32::dialog::set_text(hwnd, IDC_GROUP_READOUTS, text.group_readouts);
    win32::dialog::set_text(hwnd, IDC_HINT_READOUTS, text.hint_readouts);
    win32::dialog::set_text(hwnd, IDC_GROUP_TRAY, text.group_tray);
    win32::dialog::set_text(hwnd, IDC_HINT_TASKBAR, text.hint_taskbar);
    win32::dialog::set_text(hwnd, IDC_GROUP_UPDATE, text.group_update);
    win32::dialog::set_text(hwnd, IDC_HINT_UPDATE, text.hint_update);
    win32::dialog::set_text(hwnd, IDC_LABEL_INTERVAL, text.label_interval);
    win32::dialog::set_text(hwnd, IDC_INTERVAL_UNIT, text.unit_ms);
    win32::dialog::set_text(hwnd, IDC_GROUP_APPEARANCE, text.group_appearance);
    win32::dialog::set_text(hwnd, IDC_HINT_APPEARANCE, text.hint_appearance);
    win32::dialog::set_text(hwnd, IDC_COLOR_METRIC, text.color_metric);
    win32::dialog::set_text(hwnd, IDC_COLOR_THEME, text.color_theme);
    win32::dialog::set_text(hwnd, IDC_LABEL_ICON_COLOR, text.label_icon_color);
    win32::dialog::set_text(hwnd, IDC_LABEL_WINDOW_THEME, text.label_window_theme);
    win32::dialog::set_text(hwnd, IDC_THEME_AUTO, text.theme_auto);
    win32::dialog::set_text(hwnd, IDC_THEME_LIGHT, text.theme_light);
    win32::dialog::set_text(hwnd, IDC_THEME_DARK, text.theme_dark);
    // 그룹 상자는 `SS_NOPREFIX`가 없어 `&`를 니모닉으로 읽는다("Startup _language").
    win32::dialog::set_text(
        hwnd,
        IDC_GROUP_STARTUP,
        &text.group_startup.replace('&', "&&"),
    );
    win32::dialog::set_text(hwnd, IDC_HINT_STARTUP, text.hint_startup);
    win32::dialog::set_text(hwnd, IDC_GROUP_DEVICE, text.group_device);
    win32::dialog::set_text(hwnd, IDC_LABEL_DEVICE, text.label_device);
    win32::dialog::set_text(hwnd, IDC_AUTOSTART, text.autostart);
    win32::dialog::set_text(hwnd, IDC_LABEL_LANGUAGE, text.label_language);
    win32::dialog::set_text(hwnd, IDC_HINT_LANGUAGE, text.hint_language);
    win32::dialog::set_text(hwnd, IDC_RESET, text.reset);
    win32::dialog::set_text(hwnd, IDC_OPEN_TASKBAR, text.open_taskbar_settings);
    win32::dialog::set_text(hwnd, IDC_CLOSE, text.close);

    if RENDERED.with(Cell::get) != Some(i18n::language()) {
        fill_interval_combo(hwnd);
        fill_language_combo(hwnd, draft.language);
        fill_nav(hwnd);
        RENDERED.with(|slot| slot.set(Some(i18n::language())));
    } else {
        // 언어는 그대로지만 선택은 값에 맞춘다.
        win32::dialog::combo_select(hwnd, IDC_LANGUAGE, draft.language.index() as i32);
    }
}

/// 왼쪽 목록을 지금 언어로 다시 채우고 지금 페이지를 고른다.
fn fill_nav(hwnd: HWND) {
    let titles: Vec<&str> = Page::ALL.iter().map(|page| page.title()).collect();
    win32::dialog::list_reset(hwnd, IDC_NAV, &titles);
    win32::dialog::list_select(hwnd, IDC_NAV, page().index() as i32);
}

/// 주기 콤보를 프리셋 + `Custom…`으로 다시 채운다.
fn fill_interval_combo(hwnd: HWND) {
    win32::dialog::combo_reset(hwnd, IDC_INTERVAL);
    for ms in model::INTERVAL_PRESETS {
        win32::dialog::combo_add(hwnd, IDC_INTERVAL, &i18n::interval_label(ms));
    }
    win32::dialog::combo_add(hwnd, IDC_INTERVAL, i18n::t().custom_interval);
}

/// 언어 콤보를 다시 채우고 지금 값을 고른다.
fn fill_language_combo(hwnd: HWND, language: Language) {
    win32::dialog::combo_reset(hwnd, IDC_LANGUAGE);
    for choice in Language::ALL {
        win32::dialog::combo_add(hwnd, IDC_LANGUAGE, &i18n::language_name(choice));
    }
    win32::dialog::combo_select(hwnd, IDC_LANGUAGE, language.index() as i32);
}

/// 컨트롤을 값에 맞춘다.
fn sync_controls(hwnd: HWND, draft: &SettingsDraft) {
    apply_text(hwnd, draft);
    win32::dialog::set_check(
        hwnd,
        IDC_COLOR_METRIC,
        draft.color_mode == ColorMode::Metric,
    );
    win32::dialog::set_check(hwnd, IDC_COLOR_THEME, draft.color_mode == ColorMode::Theme);
    for (id, choice) in THEME_CONTROLS {
        win32::dialog::set_check(hwnd, id, draft.ui_theme == choice);
    }
    win32::dialog::set_check(hwnd, IDC_AUTOSTART, draft.autostart);
    sync_interval(hwnd, draft.poll_interval_ms);
    sync_page(hwnd, draft);
}

/// 페이지를 바꾼다. 컨트롤을 보이고 숨긴 뒤 그 페이지의 값을 채운다.
fn show_page(hwnd: HWND, page: Page, draft: &SettingsDraft) {
    STATE.with(|state| state.borrow_mut().page = Some(page));
    win32::dialog::list_select(hwnd, IDC_NAV, page.index() as i32);

    let general = page == Page::General;
    for id in GENERAL_CONTROLS {
        win32::dialog::show_control(hwnd, id, general);
    }
    for id in METRIC_CONTROLS {
        win32::dialog::show_control(hwnd, id, !general);
    }

    if !general {
        let kind = page.device();
        // 지표가 하나뿐인 페이지는 두 번째 체크박스를 숨긴다.
        win32::dialog::show_control(hwnd, IDC_SHOW_2, page.metrics().len() > 1);
        // 고를 장치가 없는 페이지는 콤보 대신 정보 줄만 둔다.
        let selectable = kind != DeviceKind::Fixed;
        win32::dialog::show_control(hwnd, IDC_LABEL_DEVICE, selectable);
        win32::dialog::show_control(hwnd, IDC_DEVICE, selectable);

        let layout = pages::device_layout(kind);
        win32::dialog::place_control(
            hwnd,
            IDC_GROUP_DEVICE,
            104,
            pages::DEVICE_GROUP_TOP,
            289,
            layout.group_height,
        );
        win32::dialog::place_control(hwnd, IDC_DEVICE_INFO, 112, layout.info_y, 273, 9);
        // 장치 목록은 페이지를 열 때 새로 읽는다(USB를 꽂았을 수 있다).
        fill_device_combo(hwnd, page, draft);
    }

    sync_page(hwnd, draft);
    // 숨긴 컨트롤의 자국과 제목 색을 지운다.
    win32::dialog::invalidate(hwnd);
}

/// 지금 페이지의 제목·체크박스·정보 줄을 값에 맞춘다.
fn sync_page(hwnd: HWND, draft: &SettingsDraft) {
    let page = page();
    win32::dialog::set_text(hwnd, IDC_PAGE_TITLE, page.title());
    win32::dialog::set_text(hwnd, IDC_PAGE_HINT, page.hint());
    if page == Page::General {
        return;
    }

    for (slot, id) in SHOW_CONTROLS.iter().enumerate() {
        if let Some(metric) = page.metrics().get(slot) {
            win32::dialog::set_text(hwnd, *id, i18n::metric_label(*metric));
            win32::dialog::set_check(hwnd, *id, draft.show.get(*metric));
        }
    }
    win32::dialog::set_text(hwnd, IDC_HINT_DEVICE, page.device_hint());

    // 장치 콤보의 선택은 값에 맞춘다(되돌리기·트레이 메뉴).
    if page.device() != DeviceKind::Fixed {
        let saved = device_value(page, draft);
        let in_list = STATE.with(|state| {
            state
                .borrow()
                .choices
                .as_ref()
                .and_then(|choices| choices.items.iter().position(|item| item.value == saved))
        });
        match in_list {
            Some(index) => win32::dialog::combo_select(hwnd, IDC_DEVICE, index as i32),
            // 처음 보는 값이면 목록을 다시 만든다.
            None => fill_device_combo(hwnd, page, draft),
        }
    }
    win32::dialog::set_text(hwnd, IDC_DEVICE_INFO, &device_info(page, draft));
}

/// 페이지의 장치 설정 값. `Fixed` 페이지는 없다.
fn device_value(page: Page, draft: &SettingsDraft) -> String {
    match page.device() {
        DeviceKind::Fixed => String::new(),
        DeviceKind::Gpu => draft.devices.gpu.clone(),
        DeviceKind::Network => draft.devices.network.clone(),
        DeviceKind::Disk => metrics::disk::parse_letter(&draft.devices.disk)
            .map(metrics::disk::letter_value)
            .unwrap_or_default(),
    }
}

/// 장치 콤보를 이 페이지의 목록으로 채운다.
fn fill_device_combo(hwnd: HWND, page: Page, draft: &SettingsDraft) {
    let choices = match page.device() {
        DeviceKind::Fixed => return,
        DeviceKind::Gpu => {
            let adapters = metrics::gpu::adapters();
            let primary = metrics::gpu::pick(adapters.clone(), "").map(|adapter| adapter.name);
            let names: Vec<String> = adapters.into_iter().map(|adapter| adapter.name).collect();
            pages::gpu_choices(&names, primary.as_deref(), &draft.devices.gpu)
        }
        DeviceKind::Network => {
            pages::network_choices(&metrics::network::interfaces(), &draft.devices.network)
        }
        DeviceKind::Disk => pages::disk_choices(
            &metrics::disk::drives(),
            metrics::disk::system_letter(),
            &draft.devices.disk,
        ),
    };

    win32::dialog::combo_reset(hwnd, IDC_DEVICE);
    for item in &choices.items {
        win32::dialog::combo_add(hwnd, IDC_DEVICE, &item.label);
    }
    win32::dialog::combo_select(hwnd, IDC_DEVICE, choices.selected as i32);
    STATE.with(|state| state.borrow_mut().choices = Some(choices));
}

/// 장치 그룹 맨 아래 정보 줄. 지금 무엇을 재고 있는지 보여 준다.
fn device_info(page: Page, draft: &SettingsDraft) -> String {
    match page {
        Page::General => String::new(),
        Page::Cpu => {
            let name = metrics::cpu::name().unwrap_or_else(|| "CPU".to_string());
            i18n::info_cpu(&name, metrics::cpu::logical_processors())
        }
        Page::Memory => metrics::memory::read()
            .map(|memory| i18n::info_memory(&format::bytes(memory.total)))
            .unwrap_or_default(),
        Page::Gpu => match metrics::gpu::pick(metrics::gpu::adapters(), &draft.devices.gpu) {
            Some(adapter) => {
                let vram = i18n::info_gpu(&format::bytes(adapter.dedicated_vram));
                format!("{} · {vram}", adapter.name)
            }
            None => i18n::t().info_no_gpu.to_string(),
        },
        Page::Network => {
            let device = draft.devices.network.trim();
            let interfaces = metrics::network::interfaces();
            match interfaces.iter().find(|entry| entry.alias == device) {
                Some(entry) => entry.description.clone(),
                None => {
                    let counted = metrics::network::adapters()
                        .iter()
                        .filter(|adapter| adapter.counted)
                        .count();
                    i18n::info_network_all(counted)
                }
            }
        }
        Page::Disk => {
            let letter = metrics::disk::resolve(metrics::disk::parse_letter(&draft.devices.disk));
            let model = metrics::disk::disks_of(letter)
                .first()
                .and_then(|disk| metrics::disk::model(*disk))
                .unwrap_or_else(|| metrics::disk::letter_value(letter));
            match metrics::disk::usage(letter) {
                Some(usage) => i18n::info_disk(
                    &model,
                    &format::bytes(usage.used),
                    &format::bytes(usage.total),
                ),
                None => model,
            }
        }
    }
}

/// 콤보와 입력 칸을 주기에 맞춘다. 프리셋이 아니면 `Custom…`을 고른다.
///
/// 입력 칸은 항상 켜 둔다: 콤보로 대충 고르고 숫자로 다듬는 것이 자연스럽다.
fn sync_interval(hwnd: HWND, ms: u64) {
    let index = if model::is_preset(ms) {
        model::INTERVAL_PRESETS
            .iter()
            .position(|candidate| *candidate == ms)
            .unwrap_or(0) as i32
    } else {
        custom_index()
    };

    win32::dialog::combo_select(hwnd, IDC_INTERVAL, index);
    win32::dialog::set_text(hwnd, IDC_INTERVAL_EDIT, &ms.to_string());
}

fn set_status(hwnd: HWND, message: &str) {
    win32::dialog::set_text(hwnd, IDC_STATUS, message);
}

/// Windows 작업 표시줄 설정을 연다.
///
/// 아이콘을 트레이에 고정할지는 사용자가 정한다. 앱은 그 상태를 읽지 않고, 설정 페이지를
/// 열어 줄 뿐이다.
fn open_taskbar_settings() {
    if !win32::open_url(TASKBAR_SETTINGS_URI) {
        logging::warn("could not open the taskbar settings");
    }
}

/// 창 테마 라디오를 골랐을 때. 적용한 뒤 창 전체를 새 테마로 다시 칠한다.
fn theme_selected(hwnd: HWND, id: i32) {
    let Some(choice) = THEME_CONTROLS
        .iter()
        .find(|(control, _)| *control == id)
        .map(|(_, choice)| *choice)
    else {
        return;
    };
    commit(hwnd, move |draft| {
        draft.ui_theme = choice;
        Ok(())
    });
    apply_theme(hwnd);
}

/// 왼쪽 목록에서 페이지를 골랐을 때.
fn page_selected(hwnd: HWND) {
    let selection = win32::dialog::list_selection(hwnd, IDC_NAV);
    let Some(page) = usize::try_from(selection).ok().and_then(Page::from_index) else {
        return;
    };
    if page == self::page() {
        return;
    }
    let draft = current().unwrap_or_else(|| SettingsDraft::from_config(&Config::default(), false));
    show_page(hwnd, page, &draft);
    set_status(hwnd, "");
}

/// 장치 콤보에서 골랐을 때. 고른 위치를 저장 값으로 바꿔 적용한다.
fn device_selected(hwnd: HWND) {
    let page = page();
    let selection = win32::dialog::combo_selection(hwnd, IDC_DEVICE);
    let Some(value) = STATE.with(|state| {
        let state = state.borrow();
        let choices = state.choices.as_ref()?;
        let index = usize::try_from(selection).ok()?;
        choices.items.get(index).map(|item| item.value.clone())
    }) else {
        return;
    };

    commit(hwnd, move |draft| {
        match page.device() {
            DeviceKind::Fixed => {}
            DeviceKind::Gpu => draft.devices.gpu = value,
            DeviceKind::Network => draft.devices.network = value,
            DeviceKind::Disk => draft.devices.disk = value,
        }
        Ok(())
    });
}

/// 체크박스 하나가 바뀌었을 때. 마지막 남은 지표는 거절된다.
fn toggle_metric(hwnd: HWND, id: i32, metric: Metric) {
    let checked = win32::dialog::is_check(hwnd, id);
    commit(hwnd, |draft| {
        model::set_visible(&mut draft.show, metric, checked)
            .map_err(|model::LastReadout| i18n::t().last_readout_error.to_string())
    });
}

/// 콤보에서 주기를 골랐을 때. `Custom…`이면 입력 칸의 값을 그대로 쓴다.
fn interval_selected(hwnd: HWND) {
    let selection = win32::dialog::combo_selection(hwnd, IDC_INTERVAL);
    if selection == custom_index() {
        interval_typed(hwnd);
        return;
    }

    let Some(ms) = model::INTERVAL_PRESETS
        .get(usize::try_from(selection).unwrap_or(usize::MAX))
        .copied()
    else {
        return;
    };

    commit(hwnd, move |draft| {
        draft.poll_interval_ms = ms;
        Ok(())
    });
}

/// 입력 칸에 주기를 직접 넣었을 때. 숫자가 아니면 지금 값으로 되돌린다.
fn interval_typed(hwnd: HWND) {
    let text = win32::dialog::text(hwnd, IDC_INTERVAL_EDIT);
    let Ok(ms) = text.trim().parse::<u64>() else {
        if let Some(applied) = current() {
            sync_interval(hwnd, applied.poll_interval_ms);
        }
        set_status(hwnd, i18n::t().interval_range_error);
        return;
    };

    commit(hwnd, move |draft| {
        draft.poll_interval_ms = ms;
        Ok(())
    });
}

/// 언어 콤보를 바꿨을 때. 즉시 반영되고 창 글자도 그 자리에서 바뀐다.
fn language_selected(hwnd: HWND) {
    let selection = win32::dialog::combo_selection(hwnd, IDC_LANGUAGE);
    let Some(language) = usize::try_from(selection)
        .ok()
        .and_then(Language::from_index)
    else {
        return;
    };

    commit(hwnd, move |draft| {
        draft.language = language;
        Ok(())
    });
    // 장치 콤보의 "자동" 문구도 새 언어로 만든다.
    if let Some(draft) = current() {
        let page = page();
        if page != Page::General {
            fill_device_combo(hwnd, page, &draft);
        }
    }
}

/// 설정 파일에 있는 값을 기본값으로 되돌린다.
///
/// 자동 실행은 레지스트리가 원본이라 건드리지 않는다 — 사용자가 켜 둔 것을 창에서
/// 되돌리는 것은 의도가 다르다.
fn reset(hwnd: HWND) {
    let Some(applied) = current() else {
        return;
    };

    let defaults = SettingsDraft::from_config(&Config::default(), applied.autostart);
    let effects = model::diff(&applied, &defaults);
    let problems = runtime::with(|app| app.apply_effects(&effects)).unwrap_or_default();

    sync_controls(hwnd, &defaults);

    let mut message = i18n::t().reset_done.to_string();
    if !problems.is_empty() {
        message.push_str(&format!(" · {}", problems.join(" · ")));
    }
    set_status(hwnd, &message);
}

#[cfg(test)]
mod tests {
    use super::*;
    use windows::Win32::UI::WindowsAndMessaging::IDCANCEL;

    #[test]
    fn close_button_matches_the_system_id() {
        assert_eq!(
            IDC_CLOSE, IDCANCEL.0,
            "닫기 버튼은 Esc와 같은 id를 써야 한다"
        );
    }

    /// 창이 없으면 포커스할 것도 없다. 호출자는 이때만 새 창을 연다.
    #[test]
    fn focus_reports_a_closed_window() {
        WINDOW.with(|slot| slot.set(None));
        assert!(!focus());
    }

    /// 분야 페이지의 지표는 체크박스 수를 넘지 않는다.
    #[test]
    fn every_page_fits_the_show_controls() {
        for page in Page::ALL {
            assert!(page.metrics().len() <= SHOW_CONTROLS.len(), "{page:?}");
        }
    }

    #[test]
    fn theme_controls_follow_the_theme_order() {
        let order: Vec<UiTheme> = THEME_CONTROLS.iter().map(|(_, theme)| *theme).collect();
        assert_eq!(order, UiTheme::ALL.to_vec());
    }

    /// 한 컨트롤이 두 페이지 묶음에 동시에 들어 있으면 페이지를 바꿔도 남는다.
    #[test]
    fn page_control_sets_are_disjoint() {
        for id in GENERAL_CONTROLS {
            assert!(!METRIC_CONTROLS.contains(&id), "{id}");
        }
    }

    /// `.rc`의 컨트롤 한 줄: (종류, x, y, 폭, 높이).
    fn resource_rect(resource: &str, name: &str) -> (String, i32, i32, i32, i32) {
        let line = resource
            .lines()
            .map(str::trim)
            .find(|line| {
                !line.starts_with("#define")
                    && line
                        .split(',')
                        .any(|part| part.split_whitespace().last() == Some(name))
            })
            .unwrap_or_else(|| panic!("{name}가 app.rc에 없다"));
        let kind = line
            .split_whitespace()
            .next()
            .unwrap_or_default()
            .to_string();
        let kind = if kind == "CONTROL" && line.contains("\"Button\"") {
            "BUTTON".to_string()
        } else {
            kind
        };
        let numbers: Vec<i32> = line
            .split(',')
            .filter_map(|part| part.trim().parse().ok())
            .collect();
        (kind, numbers[0], numbers[1], numbers[2], numbers[3])
    }

    /// 그룹 안의 모든 컨트롤이 같은 격자를 따른다(`resources/app.rc`의 주석).
    ///
    /// 설명은 그룹 위 +11, 줄은 +24부터 14 간격이고, 마지막 줄 아래 여백은 7이다. 좌우 여백은
    /// 8이다. 하나라도 어긋나면 그룹마다 여백이 달라 보인다.
    #[test]
    fn groups_follow_one_grid() {
        use crate::settings::pages::grid;

        let resource = include_str!("../../resources/app.rc");
        let groups: [(&str, &str, &[&str]); 6] = [
            (
                "IDC_GROUP_UPDATE",
                "IDC_HINT_UPDATE",
                &[
                    "IDC_LABEL_INTERVAL",
                    "IDC_INTERVAL",
                    "IDC_INTERVAL_EDIT",
                    "IDC_INTERVAL_UNIT",
                ],
            ),
            (
                "IDC_GROUP_APPEARANCE",
                "IDC_HINT_APPEARANCE",
                &[
                    "IDC_LABEL_ICON_COLOR",
                    "IDC_COLOR_METRIC",
                    "IDC_COLOR_THEME",
                    "IDC_LABEL_WINDOW_THEME",
                    "IDC_THEME_AUTO",
                    "IDC_THEME_LIGHT",
                    "IDC_THEME_DARK",
                ],
            ),
            (
                "IDC_GROUP_STARTUP",
                "IDC_HINT_STARTUP",
                &[
                    "IDC_AUTOSTART",
                    "IDC_LABEL_LANGUAGE",
                    "IDC_LANGUAGE",
                    "IDC_HINT_LANGUAGE",
                ],
            ),
            ("IDC_GROUP_TRAY", "IDC_HINT_TASKBAR", &["IDC_OPEN_TASKBAR"]),
            (
                "IDC_GROUP_READOUTS",
                "IDC_HINT_READOUTS",
                &["IDC_SHOW_1", "IDC_SHOW_2"],
            ),
            (
                "IDC_GROUP_DEVICE",
                "IDC_HINT_DEVICE",
                &["IDC_LABEL_DEVICE", "IDC_DEVICE", "IDC_DEVICE_INFO"],
            ),
        ];

        for (group, hint, members) in groups {
            let (_, gx, gy, gw, gh) = resource_rect(resource, group);
            assert_eq!((gx, gw), (104, 289), "{group}: 그룹 폭");
            let (left, right) = (gx + 8, gx + gw - 8);

            let (_, hx, hy, hw, _) = resource_rect(resource, hint);
            assert_eq!(
                (hx, hy, hw),
                (left, gy + grid::HINT, right - left),
                "{hint}"
            );

            let mut rows = 0;
            for name in members {
                let (kind, x, y, w, h) = resource_rect(resource, name);
                assert!(x >= left && x + w <= right, "{name}: 좌우 여백 밖");
                // 종류마다 줄 위에서 떨어진 거리가 정해져 있다.
                let offset = match kind.as_str() {
                    "LTEXT" => grid::TEXT_OFFSET,
                    "BUTTON" => 1,
                    _ => 0,
                };
                let relative = y - offset - gy - grid::FIRST_ROW;
                assert!(
                    relative >= 0 && relative % grid::ROW_PITCH == 0,
                    "{name}: y={y}는 {group}의 줄에 맞지 않는다"
                );
                // 입력 칸과 버튼은 줄 높이를 쓴다. 콤보의 높이는 펼친 목록이라 보지 않는다.
                if matches!(kind.as_str(), "EDITTEXT" | "PUSHBUTTON") {
                    assert_eq!(h, grid::ROW_HEIGHT, "{name}: 줄 높이");
                }
                rows = rows.max(relative / grid::ROW_PITCH + 1);
            }
            assert_eq!(gh, grid::group_height(rows), "{group}: 아래 여백");
        }
    }

    /// 그룹 사이 간격은 6이고, 페이지마다 첫 그룹은 같은 y에서 시작한다.
    #[test]
    fn groups_are_evenly_spaced() {
        let resource = include_str!("../../resources/app.rc");
        for page in [
            &[
                "IDC_GROUP_UPDATE",
                "IDC_GROUP_APPEARANCE",
                "IDC_GROUP_STARTUP",
                "IDC_GROUP_TRAY",
            ][..],
            &["IDC_GROUP_READOUTS", "IDC_GROUP_DEVICE"][..],
        ] {
            let rects: Vec<_> = page
                .iter()
                .map(|name| resource_rect(resource, name))
                .collect();
            assert_eq!(rects[0].2, 36, "{}", page[0]);
            for (pair, names) in rects.windows(2).zip(page.windows(2)) {
                let (above, below) = (&pair[0], &pair[1]);
                assert_eq!(
                    below.2,
                    above.2 + above.4 + 6,
                    "{} -> {}",
                    names[0],
                    names[1]
                );
            }
        }
    }

    /// `.rc`와 Rust 상수가 어긋나면 클릭이 조용히 무시된다.
    #[test]
    fn control_ids_match_the_resource_file() {
        let resource = include_str!("../../resources/app.rc");
        let declared: Vec<String> = resource
            .lines()
            .filter_map(|line| {
                let rest = line.trim().strip_prefix("#define ")?;
                let mut parts = rest.split_whitespace();
                Some(format!("{} {}", parts.next()?, parts.next()?))
            })
            .collect();

        let expected = [
            ("IDD_SETTINGS", IDD_SETTINGS as i32),
            ("IDC_SHOW_1", IDC_SHOW_1),
            ("IDC_SHOW_2", IDC_SHOW_2),
            ("IDC_NAV", IDC_NAV),
            ("IDC_PAGE_TITLE", IDC_PAGE_TITLE),
            ("IDC_INTERVAL", IDC_INTERVAL),
            ("IDC_INTERVAL_EDIT", IDC_INTERVAL_EDIT),
            ("IDC_INTERVAL_UNIT", IDC_INTERVAL_UNIT),
            ("IDC_COLOR_METRIC", IDC_COLOR_METRIC),
            ("IDC_COLOR_THEME", IDC_COLOR_THEME),
            ("IDC_AUTOSTART", IDC_AUTOSTART),
            ("IDC_STATUS", IDC_STATUS),
            ("IDC_VERSION", IDC_VERSION),
            ("IDC_PAGE_HINT", IDC_PAGE_HINT),
            ("IDC_SEPARATOR", IDC_SEPARATOR),
            ("IDC_RESET", IDC_RESET),
            ("IDC_GROUP_READOUTS", IDC_GROUP_READOUTS),
            ("IDC_HINT_READOUTS", IDC_HINT_READOUTS),
            ("IDC_GROUP_UPDATE", IDC_GROUP_UPDATE),
            ("IDC_HINT_UPDATE", IDC_HINT_UPDATE),
            ("IDC_GROUP_APPEARANCE", IDC_GROUP_APPEARANCE),
            ("IDC_HINT_APPEARANCE", IDC_HINT_APPEARANCE),
            ("IDC_GROUP_STARTUP", IDC_GROUP_STARTUP),
            ("IDC_HINT_STARTUP", IDC_HINT_STARTUP),
            ("IDC_LABEL_INTERVAL", IDC_LABEL_INTERVAL),
            ("IDC_LABEL_LANGUAGE", IDC_LABEL_LANGUAGE),
            ("IDC_LANGUAGE", IDC_LANGUAGE),
            ("IDC_HINT_LANGUAGE", IDC_HINT_LANGUAGE),
            ("IDC_HINT_TASKBAR", IDC_HINT_TASKBAR),
            ("IDC_OPEN_TASKBAR", IDC_OPEN_TASKBAR),
            ("IDC_GROUP_TRAY", IDC_GROUP_TRAY),
            ("IDC_GROUP_DEVICE", IDC_GROUP_DEVICE),
            ("IDC_HINT_DEVICE", IDC_HINT_DEVICE),
            ("IDC_LABEL_DEVICE", IDC_LABEL_DEVICE),
            ("IDC_DEVICE", IDC_DEVICE),
            ("IDC_DEVICE_INFO", IDC_DEVICE_INFO),
            ("IDC_LABEL_ICON_COLOR", IDC_LABEL_ICON_COLOR),
            ("IDC_LABEL_WINDOW_THEME", IDC_LABEL_WINDOW_THEME),
            ("IDC_THEME_AUTO", IDC_THEME_AUTO),
            ("IDC_THEME_LIGHT", IDC_THEME_LIGHT),
            ("IDC_THEME_DARK", IDC_THEME_DARK),
        ];

        for (name, value) in expected {
            let wanted = format!("{name} {value}");
            assert!(
                declared.iter().any(|entry| entry == &wanted),
                "resources/app.rc에 `#define {wanted}`가 없다"
            );
        }
    }
}
