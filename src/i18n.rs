//! 사용자에게 보이는 문자열.
//!
//! 영어 / 한국어 / 일본어 표를 `const`로 들고 있다가 지금 언어의 표를 돌려준다. 필드를
//! 빠뜨리면 컴파일이 실패하므로 세 언어가 항상 같은 키를 갖는다. 문자열이 들어오는 곳은
//! **이 파일 하나뿐**이다 — 로그 문구의 영문 검사(`logging::tests::source_strings_are_english`)
//! 가 이 파일만 예외로 두는 이유다.
//!
//! 치환은 `{0}`·`{1}` 자리표시자로 한다. 언어마다 어순이 달라도 같은 자리표시자 집합을
//! 쓰면 되고, 자리표시자 개수가 어긋나면 테스트가 잡는다.

use std::cell::Cell;

use serde::{Deserialize, Serialize};

use crate::icon::Metric;

/// 사용자가 고른 언어. `config.toml`에 이 값이 그대로 저장된다.
///
/// `Auto`를 저장하는 것이 중요하다. 해석된 언어를 저장하면 Windows 표시 언어를 바꿔도
/// `Auto`가 고정돼 따라가지 않는다.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Language {
    /// Windows 표시 언어를 따른다.
    #[default]
    Auto,
    En,
    Ko,
    Ja,
}

impl Language {
    /// 언어 콤보의 순서.
    pub const ALL: [Language; 4] = [Language::Auto, Language::En, Language::Ko, Language::Ja];

    /// 콤보에서 고른 위치.
    pub fn index(self) -> usize {
        Language::ALL
            .iter()
            .position(|candidate| *candidate == self)
            .unwrap_or(0)
    }

    /// 콤보 위치에서 언어로. 범위를 벗어나면 `None`.
    pub fn from_index(index: usize) -> Option<Self> {
        Language::ALL.get(index).copied()
    }
}

/// 해석된 언어. 문자열 표를 고르는 키다.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lang {
    En,
    Ko,
    Ja,
}

impl Lang {
    /// 콤보에 그대로 쓰는 이름. 자기 언어로 표기한다.
    pub fn native_name(self) -> &'static str {
        match self {
            Lang::En => "English",
            Lang::Ko => "한국어",
            Lang::Ja => "日本語",
        }
    }

    /// 표를 고른다.
    pub fn text(self) -> &'static Text {
        match self {
            Lang::En => &EN,
            Lang::Ko => &KO,
            Lang::Ja => &JA,
        }
    }
}

/// 한 언어의 문자열 표.
///
/// `{0}`·`{1}`은 [fill]로 채운다. 포맷이 필요한 문구는 전용 함수로 감싸 두었다.
pub struct Text {
    // 창
    pub window_title: &'static str,
    pub close: &'static str,
    pub reset: &'static str,
    pub reset_done: &'static str,

    // 표시 항목
    pub group_readouts: &'static str,
    pub hint_readouts: &'static str,
    pub hint_taskbar: &'static str,
    pub open_taskbar_settings: &'static str,
    pub metric_cpu: &'static str,
    pub metric_memory: &'static str,
    pub metric_network_down: &'static str,
    pub metric_network_up: &'static str,
    pub metric_gpu: &'static str,
    pub metric_gpu_vram: &'static str,
    pub metric_disk_usage: &'static str,
    pub metric_disk_active: &'static str,
    pub last_readout_error: &'static str,

    // 탭(왼쪽 목록)과 페이지 설명
    pub group_tray: &'static str,
    pub hint_page_general: &'static str,
    pub nav_general: &'static str,
    pub nav_cpu: &'static str,
    pub nav_memory: &'static str,
    pub nav_gpu: &'static str,
    pub nav_network: &'static str,
    pub nav_disk: &'static str,
    pub hint_page_cpu: &'static str,
    pub hint_page_memory: &'static str,
    pub hint_page_gpu: &'static str,
    pub hint_page_network: &'static str,
    pub hint_page_disk: &'static str,

    // 장치
    pub group_device: &'static str,
    pub label_device: &'static str,
    pub hint_device_cpu: &'static str,
    pub hint_device_memory: &'static str,
    pub hint_device_gpu: &'static str,
    pub hint_device_network: &'static str,
    pub hint_device_disk: &'static str,
    pub device_gpu_auto: &'static str,
    pub device_network_all: &'static str,
    pub device_disk_auto: &'static str,
    pub device_missing: &'static str,
    pub device_offline: &'static str,
    pub drive_local: &'static str,
    pub drive_removable: &'static str,
    pub info_cpu: &'static str,
    pub info_memory: &'static str,
    pub info_gpu: &'static str,
    pub info_no_gpu: &'static str,
    pub info_network_all: &'static str,
    pub info_disk: &'static str,

    // 갱신 주기
    pub group_update: &'static str,
    pub hint_update: &'static str,
    pub label_interval: &'static str,
    pub unit_ms: &'static str,
    pub custom_interval: &'static str,
    pub interval_one_second: &'static str,
    pub interval_seconds: &'static str,
    pub interval_ms: &'static str,
    pub interval_range_error: &'static str,

    // 모양
    pub group_appearance: &'static str,
    pub hint_appearance: &'static str,
    pub color_metric: &'static str,
    pub color_theme: &'static str,
    pub label_icon_color: &'static str,
    pub label_window_theme: &'static str,
    pub theme_auto: &'static str,
    pub theme_light: &'static str,
    pub theme_dark: &'static str,

    // 시작 및 언어
    pub group_startup: &'static str,
    pub hint_startup: &'static str,
    pub count_virtual: &'static str,
    pub hint_virtual: &'static str,
    pub autostart: &'static str,
    pub label_language: &'static str,
    pub hint_language: &'static str,
    pub language_auto: &'static str,

    // 트레이 메뉴
    pub menu_settings: &'static str,
    pub menu_exit: &'static str,

    // 툴팁
    pub cpu_tooltip: &'static str,
    pub memory_tooltip: &'static str,
    pub network_tooltip: &'static str,
    pub gpu_tooltip: &'static str,
    pub vram_tooltip: &'static str,
    pub network_device_tooltip: &'static str,
    pub disk_usage_tooltip: &'static str,
    pub disk_active_tooltip: &'static str,

    // 시작 실패·중복 실행
    pub already_running: &'static str,
    pub start_failed: &'static str,

    // 반영 실패·값 보정
    pub save_failed: &'static str,
    pub autostart_failed: &'static str,
    pub note_interval_clamped: &'static str,
    pub note_no_readout: &'static str,
}

pub const EN: Text = Text {
    window_title: "TinyTrayManager settings",
    close: "Close",
    reset: "Reset to defaults",
    reset_done: "Reset to defaults",

    group_readouts: "Readouts",
    hint_readouts: "Choose which values appear in the taskbar.",
    hint_taskbar: "Windows hides new tray icons; turn them on under \"Other system tray icons\".",
    open_taskbar_settings: "Open taskbar settings",
    metric_cpu: "CPU",
    metric_memory: "Memory",
    metric_network_down: "Network down",
    metric_network_up: "Network up",
    metric_gpu: "GPU",
    metric_gpu_vram: "GPU VRAM",
    metric_disk_usage: "Disk usage",
    metric_disk_active: "Disk activity",
    last_readout_error: "The last readout cannot be turned off.",

    group_tray: "Taskbar",
    hint_page_general: "Settings shared by every readout.",
    nav_general: "General",
    nav_cpu: "CPU",
    nav_memory: "Memory",
    nav_gpu: "GPU",
    nav_network: "Network",
    nav_disk: "Disk",
    hint_page_cpu: "Processor load across every core.",
    hint_page_memory: "Share of physical memory in use.",
    hint_page_gpu: "Load of the busiest engine and the dedicated memory in use.",
    hint_page_network: "Bytes received and sent per second.",
    hint_page_disk: "Space used on a drive and the share of time its disk is busy.",

    group_device: "Device",
    label_device: "Measure",
    hint_device_cpu: "Every processor is measured together.",
    hint_device_memory: "Physical memory of this PC.",
    hint_device_gpu: "The graphics adapter to measure.",
    hint_device_network: "Count every adapter together, or just one.",
    hint_device_disk: "The drive whose space and activity are measured.",
    device_gpu_auto: "Automatic ({0})",
    device_network_all: "All adapters",
    device_disk_auto: "Automatic (system drive {0})",
    device_missing: "{0} (not found)",
    device_offline: "{0} (disconnected)",
    drive_local: "Local Disk",
    drive_removable: "removable",
    info_cpu: "{0} · {1} logical processors",
    info_memory: "{0} installed",
    info_gpu: "Dedicated memory {0}",
    info_no_gpu: "No hardware GPU was found.",
    info_network_all: "Adapters counted: {0}",
    info_disk: "{0} · {1} used of {2}",

    group_update: "Update",
    hint_update: "How often the values are sampled (250-10000 ms).",
    label_interval: "Every",
    unit_ms: "ms",
    custom_interval: "Custom…",
    interval_one_second: "1 second",
    interval_seconds: "{0} seconds",
    interval_ms: "{0} ms",
    interval_range_error: "Enter milliseconds (250-10000).",

    group_appearance: "Appearance",
    hint_appearance: "Colours of the tray icons and the theme of this window and the tray menu.",
    color_metric: "Per readout",
    color_theme: "Taskbar theme",
    label_icon_color: "Icon text",
    label_window_theme: "Window",
    theme_auto: "Automatic",
    theme_light: "Light",
    theme_dark: "Dark",

    group_startup: "Startup & language",
    hint_startup: "Startup and the language of this window, the tray menu and the tooltips.",
    count_virtual: "Count virtual adapters",
    hint_virtual: "(VPN / Hyper-V switches)",
    autostart: "Run at sign-in",
    label_language: "Language",
    hint_language: "Applies immediately.",
    language_auto: "Automatic (system)",

    menu_settings: "Settings…",
    menu_exit: "Exit",

    cpu_tooltip: "CPU {0} busy",
    memory_tooltip: "Memory {0} used  ({1} of {2})",
    network_tooltip: "Network down {0}  up {1}",
    gpu_tooltip: "{0}  {1} busy",
    vram_tooltip: "GPU VRAM {0} used  ({1} of {2})",
    network_device_tooltip: "{0}  down {1}  up {2}",
    disk_usage_tooltip: "Disk {0} {1} used  ({2} of {3})",
    disk_active_tooltip: "Disk {0} {1} active  read {2}  write {3}",

    already_running: "{0} is already running.",
    start_failed: "{0} could not start.\n\n{1}",

    save_failed: "Could not save the settings file: {0}",
    autostart_failed: "Could not update sign-in startup: {0}",
    note_interval_clamped: "The update interval was adjusted to the allowed range ({0}-{1} ms).",
    note_no_readout: "No readout was on, so CPU was turned back on.",
};

pub const KO: Text = Text {
    window_title: "TinyTrayManager 설정",
    close: "닫기",
    reset: "기본값으로 복원",
    reset_done: "기본값으로 복원했습니다",

    group_readouts: "표시 항목",
    hint_readouts: "작업 표시줄에 표시할 값을 고릅니다.",
    hint_taskbar: "Windows는 새 아이콘을 숨깁니다. \"기타 시스템 트레이 아이콘\"에서 켜세요.",
    open_taskbar_settings: "작업 표시줄 설정 열기",
    metric_cpu: "CPU",
    metric_memory: "메모리",
    metric_network_down: "네트워크 다운로드",
    metric_network_up: "네트워크 업로드",
    metric_gpu: "GPU",
    metric_gpu_vram: "GPU VRAM",
    metric_disk_usage: "디스크 사용량",
    metric_disk_active: "디스크 활동",
    last_readout_error: "마지막 남은 항목은 끌 수 없습니다.",

    group_tray: "작업 표시줄",
    hint_page_general: "모든 항목에 함께 적용되는 설정입니다.",
    nav_general: "일반",
    nav_cpu: "CPU",
    nav_memory: "메모리",
    nav_gpu: "GPU",
    nav_network: "네트워크",
    nav_disk: "디스크",
    hint_page_cpu: "모든 코어를 합친 프로세서 사용률입니다.",
    hint_page_memory: "사용 중인 물리 메모리의 비율입니다.",
    hint_page_gpu: "가장 바쁜 엔진의 사용률과 사용 중인 전용 메모리입니다.",
    hint_page_network: "초당 받고 보낸 바이트입니다.",
    hint_page_disk: "드라이브의 공간 사용률과 디스크가 일한 시간의 비율입니다.",

    group_device: "장치",
    label_device: "대상",
    hint_device_cpu: "모든 프로세서를 함께 측정합니다.",
    hint_device_memory: "이 PC의 물리 메모리입니다.",
    hint_device_gpu: "측정할 그래픽 카드입니다.",
    hint_device_network: "모든 어댑터를 합치거나 하나만 셉니다.",
    hint_device_disk: "공간과 활동을 측정할 드라이브입니다.",
    device_gpu_auto: "자동 ({0})",
    device_network_all: "모든 어댑터",
    device_disk_auto: "자동 (시스템 드라이브 {0})",
    device_missing: "{0} (찾을 수 없음)",
    device_offline: "{0} (연결 끊김)",
    drive_local: "로컬 디스크",
    drive_removable: "이동식",
    info_cpu: "{0} · 논리 프로세서 {1}개",
    info_memory: "설치된 메모리 {0}",
    info_gpu: "전용 메모리 {0}",
    info_no_gpu: "하드웨어 GPU를 찾지 못했습니다.",
    info_network_all: "어댑터 {0}개를 합산합니다.",
    info_disk: "{0} · {2} 중 {1} 사용",

    group_update: "갱신",
    hint_update: "값을 읽는 주기입니다 (250~10000ms).",
    label_interval: "주기",
    unit_ms: "ms",
    custom_interval: "직접 입력…",
    interval_one_second: "1초",
    interval_seconds: "{0}초",
    interval_ms: "{0}ms",
    interval_range_error: "밀리초를 입력하세요 (250~10000).",

    group_appearance: "모양",
    hint_appearance: "트레이 아이콘의 색과 이 창·트레이 메뉴의 테마입니다.",
    color_metric: "항목마다 다른 색",
    color_theme: "작업 표시줄 테마",
    label_icon_color: "아이콘 글자",
    label_window_theme: "창 테마",
    theme_auto: "자동",
    theme_light: "밝게",
    theme_dark: "어둡게",

    group_startup: "시작 및 언어",
    hint_startup: "시작 프로그램과 창·트레이 메뉴·툴팁에 쓸 언어입니다.",
    count_virtual: "가상 어댑터 포함",
    hint_virtual: "(VPN / Hyper-V 스위치)",
    autostart: "로그인할 때 실행",
    label_language: "언어",
    hint_language: "바로 적용됩니다.",
    language_auto: "자동 (시스템)",

    menu_settings: "설정…",
    menu_exit: "종료",

    cpu_tooltip: "CPU {0} 사용 중",
    memory_tooltip: "메모리 {0} 사용 중  ({2} 중 {1})",
    network_tooltip: "네트워크 다운로드 {0}  업로드 {1}",
    gpu_tooltip: "{0}  {1} 사용 중",
    vram_tooltip: "GPU VRAM {0} 사용 중  ({2} 중 {1})",
    network_device_tooltip: "{0}  다운로드 {1}  업로드 {2}",
    disk_usage_tooltip: "디스크 {0} {1} 사용 중  ({3} 중 {2})",
    disk_active_tooltip: "디스크 {0} {1} 활동  읽기 {2}  쓰기 {3}",

    already_running: "{0}이(가) 이미 실행 중입니다.",
    start_failed: "{0}을(를) 시작하지 못했습니다.\n\n{1}",

    save_failed: "설정 파일을 저장하지 못했습니다: {0}",
    autostart_failed: "로그인 시 실행을 바꾸지 못했습니다: {0}",
    note_interval_clamped: "갱신 주기를 허용 범위로 조정했습니다 ({0}~{1}ms).",
    note_no_readout: "표시 항목이 하나도 없어 CPU를 다시 켰습니다.",
};

pub const JA: Text = Text {
    window_title: "TinyTrayManager の設定",
    close: "閉じる",
    reset: "既定値に戻す",
    reset_done: "既定値に戻しました",

    group_readouts: "表示項目",
    hint_readouts: "タスクバーに表示する値を選びます。",
    hint_taskbar: "Windows は新しいアイコンを隠します。「その他のシステム トレイ アイコン」でオンにします。",
    open_taskbar_settings: "タスクバーの設定を開く",
    metric_cpu: "CPU",
    metric_memory: "メモリ",
    metric_network_down: "ネットワーク下り",
    metric_network_up: "ネットワーク上り",
    metric_gpu: "GPU",
    metric_gpu_vram: "GPU VRAM",
    metric_disk_usage: "ディスク使用量",
    metric_disk_active: "ディスク アクティビティ",
    last_readout_error: "最後の 1 つはオフにできません。",

    group_tray: "タスクバー",
    hint_page_general: "すべての項目に共通する設定です。",
    nav_general: "全般",
    nav_cpu: "CPU",
    nav_memory: "メモリ",
    nav_gpu: "GPU",
    nav_network: "ネットワーク",
    nav_disk: "ディスク",
    hint_page_cpu: "すべてのコアを合わせたプロセッサ使用率です。",
    hint_page_memory: "使用中の物理メモリの割合です。",
    hint_page_gpu: "最も忙しいエンジンの使用率と使用中の専用メモリです。",
    hint_page_network: "1 秒あたりの受信・送信バイト数です。",
    hint_page_disk: "ドライブの使用領域と、ディスクが動作していた時間の割合です。",

    group_device: "デバイス",
    label_device: "対象",
    hint_device_cpu: "すべてのプロセッサをまとめて測定します。",
    hint_device_memory: "この PC の物理メモリです。",
    hint_device_gpu: "測定するグラフィックス アダプターです。",
    hint_device_network: "すべてのアダプターを合計するか、1 つだけを数えます。",
    hint_device_disk: "領域とアクティビティを測定するドライブです。",
    device_gpu_auto: "自動 ({0})",
    device_network_all: "すべてのアダプター",
    device_disk_auto: "自動 (システム ドライブ {0})",
    device_missing: "{0} (見つかりません)",
    device_offline: "{0} (切断)",
    drive_local: "ローカル ディスク",
    drive_removable: "リムーバブル",
    info_cpu: "{0} · 論理プロセッサ {1} 個",
    info_memory: "搭載メモリ {0}",
    info_gpu: "専用メモリ {0}",
    info_no_gpu: "ハードウェア GPU が見つかりません。",
    info_network_all: "{0} 個のアダプターを合計します。",
    info_disk: "{0} · {2} 中 {1} 使用",

    group_update: "更新",
    hint_update: "値を取得する間隔です (250～10000ms)。",
    label_interval: "間隔",
    unit_ms: "ms",
    custom_interval: "カスタム…",
    interval_one_second: "1 秒",
    interval_seconds: "{0} 秒",
    interval_ms: "{0} ms",
    interval_range_error: "ミリ秒を入力してください (250～10000)。",

    group_appearance: "外観",
    hint_appearance: "トレイ アイコンの色と、このウィンドウ・トレイ メニューのテーマです。",
    color_metric: "項目ごとに色分け",
    color_theme: "タスクバーのテーマ",
    label_icon_color: "アイコン文字",
    label_window_theme: "ウィンドウ",
    theme_auto: "自動",
    theme_light: "ライト",
    theme_dark: "ダーク",

    group_startup: "スタートアップと言語",
    hint_startup: "スタートアップと、このウィンドウ・トレイメニュー・ツールチップの言語です。",
    count_virtual: "仮想アダプターを含める",
    hint_virtual: "(VPN / Hyper-V スイッチ)",
    autostart: "サインイン時に実行",
    label_language: "言語",
    hint_language: "すぐに反映されます。",
    language_auto: "自動 (システム)",

    menu_settings: "設定…",
    menu_exit: "終了",

    cpu_tooltip: "CPU {0} 使用中",
    memory_tooltip: "メモリ {0} 使用中  ({2} / {1})",
    network_tooltip: "ネットワーク 下り {0}  上り {1}",
    gpu_tooltip: "{0}  {1} 使用中",
    vram_tooltip: "GPU VRAM {0} 使用中  ({2} / {1})",
    network_device_tooltip: "{0}  下り {1}  上り {2}",
    disk_usage_tooltip: "ディスク {0} {1} 使用中  ({2} / {3})",
    disk_active_tooltip: "ディスク {0} {1} 稼働  読み取り {2}  書き込み {3}",

    already_running: "{0} は既に実行中です。",
    start_failed: "{0} を起動できませんでした。\n\n{1}",

    save_failed: "設定ファイルを保存できませんでした: {0}",
    autostart_failed: "サインイン時の起動を変更できませんでした: {0}",
    note_interval_clamped: "更新間隔を許可範囲に調整しました ({0}～{1}ms)。",
    note_no_readout: "表示項目がなかったため CPU を再びオンにしました。",
};

thread_local! {
    /// 지금 언어. 시작 시 시스템 언어로 맞추고, `Config.language`가 있으면 그것으로 덮는다.
    static CURRENT: Cell<Lang> = const { Cell::new(Lang::En) };
}

/// 지금 언어의 문자열 표.
pub fn t() -> &'static Text {
    CURRENT.with(Cell::get).text()
}

/// 지금 언어.
pub fn language() -> Lang {
    CURRENT.with(Cell::get)
}

/// 언어를 바꾼다. 창·트레이 메뉴·툴팁은 각자 다시 그린다.
pub fn set_language(lang: Lang) {
    CURRENT.with(|cell| cell.set(lang));
}

/// 시스템 표시 언어로 맞춘다. `App`이 설정을 읽기 전에 쓴다.
pub fn use_system_language() -> Lang {
    let lang = from_lang_id(crate::win32::user_language_id());
    set_language(lang);
    lang
}

/// LANGID를 지원 언어로 옮긴다. 모르는 언어는 영어다.
///
/// LANGID의 하위 10비트가 주 언어 id다(`LANG_KOREAN` = 0x12, `LANG_JAPANESE` = 0x11).
pub fn from_lang_id(lang_id: u16) -> Lang {
    match lang_id & 0x03FF {
        0x12 => Lang::Ko,
        0x11 => Lang::Ja,
        _ => Lang::En,
    }
}

/// 저장된 선택을 지금 언어로 푼다. `Auto`는 시스템 언어를 따른다.
pub fn resolve(choice: Language) -> Lang {
    match choice {
        Language::Auto => from_lang_id(crate::win32::user_language_id()),
        Language::En => Lang::En,
        Language::Ko => Lang::Ko,
        Language::Ja => Lang::Ja,
    }
}

/// `{0}`·`{1}` 자리표시자를 인자로 바꾼다.
fn fill(template: &str, args: &[&str]) -> String {
    let mut out = template.to_string();
    for (index, arg) in args.iter().enumerate() {
        out = out.replace(&format!("{{{index}}}"), arg);
    }
    out
}

/// 언어 콤보에 넣는 이름. `Auto`는 지금 언어로, 나머지는 자기 언어로 적는다.
pub fn language_name(choice: Language) -> String {
    match choice {
        Language::Auto => t().language_auto.to_string(),
        Language::En => Lang::En.native_name().to_string(),
        Language::Ko => Lang::Ko.native_name().to_string(),
        Language::Ja => Lang::Ja.native_name().to_string(),
    }
}

/// 메뉴·체크박스에 쓰는 지표 이름.
pub fn metric_label(metric: Metric) -> &'static str {
    let text = t();
    match metric {
        Metric::Cpu => text.metric_cpu,
        Metric::Memory => text.metric_memory,
        Metric::NetworkDown => text.metric_network_down,
        Metric::NetworkUp => text.metric_network_up,
        Metric::Gpu => text.metric_gpu,
        Metric::GpuVram => text.metric_gpu_vram,
        Metric::DiskUsage => text.metric_disk_usage,
        Metric::DiskActive => text.metric_disk_active,
    }
}

/// 주기 콤보에 넣는 표기.
pub fn interval_label(ms: u64) -> String {
    let text = t();
    match ms {
        1_000 => text.interval_one_second.to_string(),
        _ if ms % 1_000 == 0 => fill(text.interval_seconds, &[&(ms / 1_000).to_string()]),
        _ => fill(text.interval_ms, &[&ms.to_string()]),
    }
}

/// 트레이 아이콘 툴팁.
pub fn cpu_tooltip(percent: &str) -> String {
    fill(t().cpu_tooltip, &[percent])
}

pub fn memory_tooltip(percent: &str, used: &str, total: &str) -> String {
    fill(t().memory_tooltip, &[percent, used, total])
}

pub fn network_tooltip(down: &str, up: &str) -> String {
    fill(t().network_tooltip, &[down, up])
}

/// GPU 어댑터 이름과 사용률.
pub fn gpu_tooltip(adapter: &str, percent: &str) -> String {
    fill(t().gpu_tooltip, &[adapter, percent])
}

pub fn vram_tooltip(percent: &str, used: &str, total: &str) -> String {
    fill(t().vram_tooltip, &[percent, used, total])
}

/// 고른 인터페이스 하나의 처리량.
pub fn network_device_tooltip(device: &str, down: &str, up: &str) -> String {
    fill(t().network_device_tooltip, &[device, down, up])
}

/// 드라이브(`C:`)의 공간 사용률.
pub fn disk_usage_tooltip(drive: &str, percent: &str, used: &str, total: &str) -> String {
    fill(t().disk_usage_tooltip, &[drive, percent, used, total])
}

/// 드라이브가 놓인 디스크의 활성 시간과 처리량.
pub fn disk_active_tooltip(drive: &str, percent: &str, read: &str, write: &str) -> String {
    fill(t().disk_active_tooltip, &[drive, percent, read, write])
}

/// GPU 콤보의 자동 항목. 지금 자동으로 고르는 어댑터를 괄호에 넣는다.
pub fn device_gpu_auto(adapter: &str) -> String {
    fill(t().device_gpu_auto, &[adapter])
}

/// 디스크 콤보의 자동 항목. (`C:`)
pub fn device_disk_auto(drive: &str) -> String {
    fill(t().device_disk_auto, &[drive])
}

/// 설정에는 있지만 지금 없는 장치.
pub fn device_missing(name: &str) -> String {
    fill(t().device_missing, &[name])
}

/// 있지만 연결되지 않은 네트워크 인터페이스.
pub fn device_offline(name: &str) -> String {
    fill(t().device_offline, &[name])
}

/// CPU 페이지의 정보 줄.
pub fn info_cpu(name: &str, logical: u32) -> String {
    fill(t().info_cpu, &[name, &logical.to_string()])
}

/// 메모리 페이지의 정보 줄.
pub fn info_memory(total: &str) -> String {
    fill(t().info_memory, &[total])
}

/// GPU 페이지의 정보 줄.
pub fn info_gpu(vram: &str) -> String {
    fill(t().info_gpu, &[vram])
}

/// 네트워크 페이지에서 모든 어댑터를 셀 때의 정보 줄.
pub fn info_network_all(count: usize) -> String {
    fill(t().info_network_all, &[&count.to_string()])
}

/// 디스크 페이지의 정보 줄. (모델, 사용, 전체)
pub fn info_disk(model: &str, used: &str, total: &str) -> String {
    fill(t().info_disk, &[model, used, total])
}

/// 중복 실행 알림.
pub fn already_running(product: &str) -> String {
    fill(t().already_running, &[product])
}

/// 시작 실패 알림.
pub fn start_failed(product: &str, error: &str) -> String {
    fill(t().start_failed, &[product, error])
}

/// 설정 파일 저장 실패.
pub fn save_failed(error: &str) -> String {
    fill(t().save_failed, &[error])
}

/// 자동 실행 변경 실패.
pub fn autostart_failed(error: &str) -> String {
    fill(t().autostart_failed, &[error])
}

/// 주기가 허용 범위로 조정됐다는 상태줄 문구.
pub fn note_interval_clamped(from: u64, to: u64) -> String {
    fill(
        t().note_interval_clamped,
        &[&from.to_string(), &to.to_string()],
    )
}

/// 표시 항목이 하나도 없어 CPU를 켰다는 상태줄 문구.
pub fn note_no_readout() -> String {
    t().note_no_readout.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    const TABLES: [(&str, &Text); 3] = [("EN", &EN), ("KO", &KO), ("JA", &JA)];

    /// 리터럴이 있는 필드를 전부 모은다. 새 필드를 추가하면 여기에도 추가해야 한다.
    fn strings(text: &Text) -> Vec<(&'static str, &'static str)> {
        vec![
            ("window_title", text.window_title),
            ("close", text.close),
            ("reset", text.reset),
            ("reset_done", text.reset_done),
            ("group_readouts", text.group_readouts),
            ("hint_readouts", text.hint_readouts),
            ("hint_taskbar", text.hint_taskbar),
            ("open_taskbar_settings", text.open_taskbar_settings),
            ("metric_cpu", text.metric_cpu),
            ("metric_memory", text.metric_memory),
            ("metric_network_down", text.metric_network_down),
            ("metric_network_up", text.metric_network_up),
            ("metric_gpu", text.metric_gpu),
            ("metric_gpu_vram", text.metric_gpu_vram),
            ("metric_disk_usage", text.metric_disk_usage),
            ("metric_disk_active", text.metric_disk_active),
            ("last_readout_error", text.last_readout_error),
            ("group_tray", text.group_tray),
            ("hint_page_general", text.hint_page_general),
            ("nav_general", text.nav_general),
            ("nav_cpu", text.nav_cpu),
            ("nav_memory", text.nav_memory),
            ("nav_gpu", text.nav_gpu),
            ("nav_network", text.nav_network),
            ("nav_disk", text.nav_disk),
            ("hint_page_cpu", text.hint_page_cpu),
            ("hint_page_memory", text.hint_page_memory),
            ("hint_page_gpu", text.hint_page_gpu),
            ("hint_page_network", text.hint_page_network),
            ("hint_page_disk", text.hint_page_disk),
            ("group_device", text.group_device),
            ("label_device", text.label_device),
            ("hint_device_cpu", text.hint_device_cpu),
            ("hint_device_memory", text.hint_device_memory),
            ("hint_device_gpu", text.hint_device_gpu),
            ("hint_device_network", text.hint_device_network),
            ("hint_device_disk", text.hint_device_disk),
            ("device_gpu_auto", text.device_gpu_auto),
            ("device_network_all", text.device_network_all),
            ("device_disk_auto", text.device_disk_auto),
            ("device_missing", text.device_missing),
            ("device_offline", text.device_offline),
            ("drive_local", text.drive_local),
            ("drive_removable", text.drive_removable),
            ("info_cpu", text.info_cpu),
            ("info_memory", text.info_memory),
            ("info_gpu", text.info_gpu),
            ("info_no_gpu", text.info_no_gpu),
            ("info_network_all", text.info_network_all),
            ("info_disk", text.info_disk),
            ("group_update", text.group_update),
            ("hint_update", text.hint_update),
            ("label_interval", text.label_interval),
            ("unit_ms", text.unit_ms),
            ("custom_interval", text.custom_interval),
            ("interval_one_second", text.interval_one_second),
            ("interval_seconds", text.interval_seconds),
            ("interval_ms", text.interval_ms),
            ("interval_range_error", text.interval_range_error),
            ("group_appearance", text.group_appearance),
            ("hint_appearance", text.hint_appearance),
            ("color_metric", text.color_metric),
            ("color_theme", text.color_theme),
            ("label_icon_color", text.label_icon_color),
            ("label_window_theme", text.label_window_theme),
            ("theme_auto", text.theme_auto),
            ("theme_light", text.theme_light),
            ("theme_dark", text.theme_dark),
            ("group_startup", text.group_startup),
            ("hint_startup", text.hint_startup),
            ("count_virtual", text.count_virtual),
            ("hint_virtual", text.hint_virtual),
            ("autostart", text.autostart),
            ("label_language", text.label_language),
            ("hint_language", text.hint_language),
            ("language_auto", text.language_auto),
            ("menu_settings", text.menu_settings),
            ("menu_exit", text.menu_exit),
            ("cpu_tooltip", text.cpu_tooltip),
            ("memory_tooltip", text.memory_tooltip),
            ("network_tooltip", text.network_tooltip),
            ("gpu_tooltip", text.gpu_tooltip),
            ("vram_tooltip", text.vram_tooltip),
            ("network_device_tooltip", text.network_device_tooltip),
            ("disk_usage_tooltip", text.disk_usage_tooltip),
            ("disk_active_tooltip", text.disk_active_tooltip),
            ("already_running", text.already_running),
            ("start_failed", text.start_failed),
            ("save_failed", text.save_failed),
            ("autostart_failed", text.autostart_failed),
            ("note_interval_clamped", text.note_interval_clamped),
            ("note_no_readout", text.note_no_readout),
        ]
    }

    /// `{n}` 자리표시자를 등장 순서대로 모은다.
    fn placeholders(template: &str) -> Vec<u32> {
        let chars: Vec<char> = template.chars().collect();
        let mut found = Vec::new();
        let mut i = 0;
        while i < chars.len() {
            if chars[i] == '{' {
                let start = i + 1;
                let mut end = start;
                while end < chars.len() && chars[end] != '}' {
                    end += 1;
                }
                if end < chars.len() {
                    if let Ok(index) = chars[start..end].iter().collect::<String>().parse() {
                        found.push(index);
                    }
                }
                i = end + 1;
            } else {
                i += 1;
            }
        }
        found
    }

    #[test]
    fn every_string_is_present_in_every_language() {
        for (name, text) in TABLES {
            for (key, value) in strings(text) {
                assert!(!value.is_empty(), "{name}: {key} is empty");
            }
        }
    }

    /// 언어마다 필드 집합이 같아야 한다(누락은 컴파일이 잡지만, 순서도 고정한다).
    #[test]
    fn tables_share_the_same_keys() {
        let reference: Vec<&str> = strings(&EN).iter().map(|(key, _)| *key).collect();
        for (name, text) in TABLES {
            let keys: Vec<&str> = strings(text).iter().map(|(key, _)| *key).collect();
            assert_eq!(keys, reference, "{name} table differs");
        }
    }

    /// 자리표시자 집합이 언어마다 같아야 한다. 하나라도 다르면 [fill]이 `{n}`을 남긴다.
    /// 어순이 달라도 되므로 순서는 보지 않고 집합만 본다.
    #[test]
    fn placeholders_match_across_languages() {
        fn set(mut found: Vec<u32>) -> Vec<u32> {
            found.sort_unstable();
            found.dedup();
            found
        }

        let reference: Vec<(&str, Vec<u32>)> = strings(&EN)
            .into_iter()
            .map(|(key, value)| (key, set(placeholders(value))))
            .filter(|(_, found)| !found.is_empty())
            .collect();

        for (name, text) in TABLES {
            let actual: Vec<(&str, Vec<u32>)> = strings(text)
                .into_iter()
                .map(|(key, value)| (key, set(placeholders(value))))
                .filter(|(_, found)| !found.is_empty())
                .collect();
            assert_eq!(actual, reference, "{name} placeholders differ");
        }
    }

    /// 영어 표에는 한글·일본어 문자가 없어야 한다(로그·폴백 호환). `…` 같은 부호는 허용한다.
    #[test]
    fn the_english_table_has_no_cjk() {
        for (key, value) in strings(&EN) {
            assert!(
                !value.chars().any(is_cjk),
                "EN: {key} contains CJK: {value}"
            );
        }
    }

    fn is_cjk(c: char) -> bool {
        matches!(
            c,
            '\u{AC00}'..='\u{D7A3}' // Hangul syllables
                | '\u{3131}'..='\u{318E}' // Hangul jamo
                | '\u{3040}'..='\u{309F}' // Hiragana
                | '\u{30A0}'..='\u{30FF}' // Katakana
                | '\u{4E00}'..='\u{9FFF}' // CJK ideographs
        )
    }

    #[test]
    fn fill_replaces_every_placeholder() {
        assert_eq!(fill("{0}-{1}-{0}", &["a", "b"]), "a-b-a");
        assert_eq!(fill("plain", &[]), "plain");
    }

    #[test]
    fn lang_id_maps_to_supported_languages() {
        // LANG_KOREAN = 0x12, LANG_JAPANESE = 0x11, LANG_ENGLISH = 0x09
        assert_eq!(from_lang_id(0x12), Lang::Ko);
        assert_eq!(from_lang_id(0x0412), Lang::Ko, "하위 10비트만 본다");
        assert_eq!(from_lang_id(0x11), Lang::Ja);
        assert_eq!(from_lang_id(0x0411), Lang::Ja);
        assert_eq!(from_lang_id(0x09), Lang::En);
        assert_eq!(from_lang_id(0x0407), Lang::En, "독일어는 영어로 떨어진다");
        assert_eq!(from_lang_id(0x0000), Lang::En);
    }

    #[test]
    fn language_names_are_native() {
        for lang in [Lang::En, Lang::Ko, Lang::Ja] {
            set_language(lang);
            assert_eq!(language_name(Language::En), "English");
            assert_eq!(language_name(Language::Ko), "한국어");
            assert_eq!(language_name(Language::Ja), "日本語");
            assert!(!language_name(Language::Auto).is_empty());
        }
        set_language(Lang::En);
    }

    #[test]
    fn language_index_round_trips() {
        for choice in Language::ALL {
            assert_eq!(Language::from_index(choice.index()), Some(choice));
        }
        assert_eq!(Language::from_index(4), None);
    }

    /// 채워진 문구에 `{n}`이 남지 않아야 한다.
    #[test]
    fn formatted_strings_have_no_leftover_placeholders() {
        for lang in [Lang::En, Lang::Ko, Lang::Ja] {
            set_language(lang);
            let samples = [
                interval_label(1_000),
                interval_label(2_000),
                interval_label(250),
                cpu_tooltip("12.3%"),
                memory_tooltip("60%", "9.6 GB", "16 GB"),
                network_tooltip("1.2 MB/s", "340 KB/s"),
                gpu_tooltip("NVIDIA GeForce RTX 4060", "14%"),
                vram_tooltip("87%", "6.9 GB", "7.8 GB"),
                network_device_tooltip("Ethernet", "1.2 MB/s", "340 KB/s"),
                disk_usage_tooltip("C:", "58%", "1.07 TB", "1.86 TB"),
                disk_active_tooltip("C:", "14%", "51 MB/s", "56 MB/s"),
                device_gpu_auto("NVIDIA GeForce RTX 4060"),
                device_disk_auto("C:"),
                device_missing("F:"),
                device_offline("Wi-Fi"),
                info_cpu("AMD Ryzen 9 5950X", 32),
                info_memory("64.0 GB"),
                info_gpu("8.00 GB"),
                info_network_all(3),
                info_disk("Samsung SSD", "1.07 TB", "1.86 TB"),
                already_running("TinyTrayManager"),
                start_failed("TinyTrayManager", "boom"),
                save_failed("denied"),
                autostart_failed("denied"),
                note_interval_clamped(1, 250),
                note_no_readout(),
            ];
            for sample in samples {
                assert!(
                    !sample.contains('{'),
                    "{lang:?}: leftover placeholder in {sample}"
                );
            }
        }
        set_language(Lang::En);
    }

    #[test]
    fn interval_labels_read_naturally() {
        set_language(Lang::En);
        assert_eq!(interval_label(250), "250 ms");
        assert_eq!(interval_label(1_000), "1 second");
        assert_eq!(interval_label(5_000), "5 seconds");
        assert_eq!(interval_label(750), "750 ms");

        set_language(Lang::Ko);
        assert_eq!(interval_label(250), "250ms");
        assert_eq!(interval_label(2_000), "2초");
    }

    #[test]
    fn metric_labels_cover_every_metric() {
        for lang in [Lang::En, Lang::Ko, Lang::Ja] {
            set_language(lang);
            for metric in Metric::ALL {
                assert!(!metric_label(metric).is_empty());
            }
        }
        set_language(Lang::En);
    }
}
