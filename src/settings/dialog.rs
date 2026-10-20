//! 설정 대화상자.
//!
//! `resources/app.rc`의 `IDD_SETTINGS` 템플릿을 띄운다. 값은 저장된 설정에서 읽어 오고,
//! 컨트롤이 바뀔 때마다 바뀐 항목만 적용한다 — 저장 버튼이 없다(즉시 적용).
//!
//! 템플릿의 글자는 전부 비어 있고, `WM_INITDIALOG`에서 [crate::i18n]의 표로 채운다. 그래서
//! 창을 닫지 않고 언어를 바꿀 수 있다.
//!
//! Win32 호출은 [crate::win32::dialog]에 모아 두었다. 이 모듈은 배치와 규칙만 다룬다.

use std::cell::{Cell, RefCell};

use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::{
    CBN_SELCHANGE, EN_KILLFOCUS, WM_CLOSE, WM_COMMAND, WM_INITDIALOG, WM_SETTINGCHANGE,
    WM_THEMECHANGED,
};

use crate::config::{ColorMode, Config};
use crate::i18n::{self, Lang, Language};
use crate::icon::{self, Metric};
use crate::logging;
use crate::runtime;
use crate::settings::model::{self, SettingsDraft};
use crate::settings::preview::{self, Preview};
use crate::theme;
use crate::win32;

/// 리소스의 대화상자 id.
pub const IDD_SETTINGS: u16 = 101;

// 컨트롤 id. `resources/app.rc`의 `#define`과 같아야 한다(테스트로 대조한다).
pub const IDC_CPU: i32 = 1001;
pub const IDC_MEMORY: i32 = 1002;
pub const IDC_NETWORK_DOWN: i32 = 1003;
pub const IDC_NETWORK_UP: i32 = 1004;
pub const IDC_PREVIEW: i32 = 1005;
pub const IDC_INTERVAL: i32 = 1006;
pub const IDC_INTERVAL_EDIT: i32 = 1007;
pub const IDC_INTERVAL_UNIT: i32 = 1008;
pub const IDC_COLOR_METRIC: i32 = 1009;
pub const IDC_COLOR_THEME: i32 = 1010;
pub const IDC_VIRTUAL: i32 = 1011;
pub const IDC_AUTOSTART: i32 = 1012;
pub const IDC_STATUS: i32 = 1013;
pub const IDC_VERSION: i32 = 1014;
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
pub const IDC_HINT_VIRTUAL: i32 = 1029;
pub const IDC_HINT_LANGUAGE: i32 = 1030;

/// 닫기 버튼과 Esc가 보내는 id.
pub const IDC_CLOSE: i32 = 2;

/// 체크박스와 지표. 순서는 [Metric::ALL]과 같다.
pub const SHOW_CONTROLS: [(i32, Metric); 4] = [
    (IDC_CPU, Metric::Cpu),
    (IDC_MEMORY, Metric::Memory),
    (IDC_NETWORK_DOWN, Metric::NetworkDown),
    (IDC_NETWORK_UP, Metric::NetworkUp),
];

thread_local! {
    /// 미리보기 글리프 캐시. 트레이 캐시를 공유하면 하트비트 로그의 히트율이 흐려진다.
    static PREVIEW: RefCell<Preview> = RefCell::new(Preview::default());
    /// 테마를 입히는 중인가.
    ///
    /// `SetPreferredAppMode`와 `SetWindowTheme`은 다시 테마 메시지를 보낸다. 그 메시지를
    /// 그대로 처리하면 끝없이 되돌아 스택이 넘친다(실측: build 26300에서 스택 오버플로).
    static APPLYING: Cell<bool> = const { Cell::new(false) };
    /// 열려 있는 설정 창. 트레이 메뉴가 바꾼 값을 창에 알릴 때 쓴다.
    static WINDOW: Cell<Option<HWND>> = const { Cell::new(None) };
    /// 콤보 항목을 마지막으로 채운 언어. 언어가 바뀔 때만 다시 채운다.
    static RENDERED: Cell<Option<Lang>> = const { Cell::new(None) };
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

extern "system" fn procedure(hwnd: HWND, message: u32, wparam: WPARAM, _lparam: LPARAM) -> isize {
    // 색을 돌려주는 메시지는 다크 모드일 때만 가로챈다.
    if win32::dark::is_color_message(message) {
        return win32::dark::paint(dark(), message, wparam).unwrap_or(0);
    }

    match message {
        WM_INITDIALOG => {
            on_init(hwnd);
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

/// 작업 표시줄이 어두운가.
fn dark() -> bool {
    !theme::current().light
}

/// 창을 닫는다. 다크 모드용 GDI 자원도 함께 돌려준다.
fn finish(hwnd: HWND) {
    WINDOW.with(|slot| slot.set(None));
    RENDERED.with(|slot| slot.set(None));
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

    if let Some(draft) = current() {
        sync_preview(hwnd, &draft);
    }
    win32::dialog::invalidate(hwnd);

    APPLYING.with(|flag| flag.set(false));
}

fn on_init(hwnd: HWND) {
    WINDOW.with(|slot| slot.set(Some(hwnd)));
    win32::dialog::center_on_cursor_monitor(hwnd);
    win32::dialog::bring_to_front(hwnd);
    apply_theme(hwnd);

    win32::dialog::set_text(
        hwnd,
        IDC_VERSION,
        &format!("v{}", env!("CARGO_PKG_VERSION")),
    );

    let draft = current().unwrap_or_else(|| SettingsDraft::from_config(&Config::default(), false));
    sync_controls(hwnd, &draft);
}

fn on_command(hwnd: HWND, wparam: WPARAM) {
    let id = (wparam.0 & 0xFFFF) as i32;
    let notification = ((wparam.0 >> 16) & 0xFFFF) as u32;

    if let Some(metric) = metric_for(id) {
        toggle_metric(hwnd, id, metric);
        return;
    }

    match id {
        IDC_CLOSE => finish(hwnd),
        IDC_RESET => reset(hwnd),
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
        IDC_VIRTUAL => commit(hwnd, |draft| {
            draft.count_virtual = !draft.count_virtual;
            Ok(())
        }),
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
    for (id, metric) in SHOW_CONTROLS {
        win32::dialog::set_text(hwnd, id, i18n::metric_label(metric));
    }
    win32::dialog::set_text(hwnd, IDC_GROUP_UPDATE, text.group_update);
    win32::dialog::set_text(hwnd, IDC_HINT_UPDATE, text.hint_update);
    win32::dialog::set_text(hwnd, IDC_LABEL_INTERVAL, text.label_interval);
    win32::dialog::set_text(hwnd, IDC_INTERVAL_UNIT, text.unit_ms);
    win32::dialog::set_text(hwnd, IDC_GROUP_APPEARANCE, text.group_appearance);
    win32::dialog::set_text(hwnd, IDC_HINT_APPEARANCE, text.hint_appearance);
    win32::dialog::set_text(hwnd, IDC_COLOR_METRIC, text.color_metric);
    win32::dialog::set_text(hwnd, IDC_COLOR_THEME, text.color_theme);
    win32::dialog::set_text(hwnd, IDC_GROUP_STARTUP, text.group_startup);
    win32::dialog::set_text(hwnd, IDC_HINT_STARTUP, text.hint_startup);
    win32::dialog::set_text(hwnd, IDC_VIRTUAL, text.count_virtual);
    win32::dialog::set_text(hwnd, IDC_HINT_VIRTUAL, text.hint_virtual);
    win32::dialog::set_text(hwnd, IDC_AUTOSTART, text.autostart);
    win32::dialog::set_text(hwnd, IDC_LABEL_LANGUAGE, text.label_language);
    win32::dialog::set_text(hwnd, IDC_HINT_LANGUAGE, text.hint_language);
    win32::dialog::set_text(hwnd, IDC_RESET, text.reset);
    win32::dialog::set_text(hwnd, IDC_CLOSE, text.close);

    if RENDERED.with(Cell::get) != Some(i18n::language()) {
        fill_interval_combo(hwnd);
        fill_language_combo(hwnd, draft.language);
        RENDERED.with(|slot| slot.set(Some(i18n::language())));
    } else {
        // 언어는 그대로지만 선택은 값에 맞춘다.
        win32::dialog::combo_select(hwnd, IDC_LANGUAGE, draft.language.index() as i32);
    }
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
    for (id, metric) in SHOW_CONTROLS {
        win32::dialog::set_check(hwnd, id, draft.show.get(metric));
    }
    win32::dialog::set_check(
        hwnd,
        IDC_COLOR_METRIC,
        draft.color_mode == ColorMode::Metric,
    );
    win32::dialog::set_check(hwnd, IDC_COLOR_THEME, draft.color_mode == ColorMode::Theme);
    win32::dialog::set_check(hwnd, IDC_VIRTUAL, draft.count_virtual);
    win32::dialog::set_check(hwnd, IDC_AUTOSTART, draft.autostart);
    sync_interval(hwnd, draft.poll_interval_ms);
    sync_preview(hwnd, draft);
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

/// 아이콘 미리보기를 다시 그린다. 트레이 아이콘과 같은 렌더러를 쓴다.
fn sync_preview(hwnd: HWND, draft: &SettingsDraft) {
    let palette = theme::current();
    let px = win32::tray_icon_size();
    let bitmap = PREVIEW.with(|cell| cell.borrow_mut().strip(draft, palette, px, preview::SCALE));

    match icon::gdi::bitmap_from_rgba(bitmap.width, bitmap.height, &bitmap.rgba) {
        Some(handle) => win32::dialog::set_bitmap(hwnd, IDC_PREVIEW, handle),
        None => logging::warn("could not create the preview bitmap"),
    }
}

fn set_status(hwnd: HWND, message: &str) {
    win32::dialog::set_text(hwnd, IDC_STATUS, message);
}

fn metric_for(id: i32) -> Option<Metric> {
    SHOW_CONTROLS
        .iter()
        .find(|(control, _)| *control == id)
        .map(|(_, metric)| *metric)
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

    #[test]
    fn show_controls_cover_every_metric_in_order() {
        let metrics: Vec<Metric> = SHOW_CONTROLS.iter().map(|(_, metric)| *metric).collect();
        assert_eq!(metrics, Metric::ALL.to_vec());
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
            ("IDC_CPU", IDC_CPU),
            ("IDC_MEMORY", IDC_MEMORY),
            ("IDC_NETWORK_DOWN", IDC_NETWORK_DOWN),
            ("IDC_NETWORK_UP", IDC_NETWORK_UP),
            ("IDC_PREVIEW", IDC_PREVIEW),
            ("IDC_INTERVAL", IDC_INTERVAL),
            ("IDC_INTERVAL_EDIT", IDC_INTERVAL_EDIT),
            ("IDC_INTERVAL_UNIT", IDC_INTERVAL_UNIT),
            ("IDC_COLOR_METRIC", IDC_COLOR_METRIC),
            ("IDC_COLOR_THEME", IDC_COLOR_THEME),
            ("IDC_VIRTUAL", IDC_VIRTUAL),
            ("IDC_AUTOSTART", IDC_AUTOSTART),
            ("IDC_STATUS", IDC_STATUS),
            ("IDC_VERSION", IDC_VERSION),
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
            ("IDC_HINT_VIRTUAL", IDC_HINT_VIRTUAL),
            ("IDC_HINT_LANGUAGE", IDC_HINT_LANGUAGE),
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
