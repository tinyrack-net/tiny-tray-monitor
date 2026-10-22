//! 설정 창의 페이지(왼쪽 목록)와 장치 선택지.
//!
//! 창은 `General` 페이지와 분야별 페이지로 나뉜다. 분야 페이지는 같은 틀을 쓴다: 그 분야의
//! 표시 항목과 잴 장치. 어느 컨트롤이 어느 페이지에 속하는지, 장치 콤보에 무엇을 넣는지는
//! 여기서 순수 함수로 정하고 테스트한다. Win32 호출은 [super::dialog]에만 있다.

use crate::i18n;
use crate::icon::Metric;
use crate::metrics::disk::{self, Drive};
use crate::metrics::network::Interface;

/// 설정 창의 페이지. 순서가 왼쪽 목록의 순서다.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Page {
    General,
    Cpu,
    Memory,
    Gpu,
    Network,
    Disk,
}

/// 분야 페이지가 고르는 장치의 종류.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceKind {
    /// 고를 것이 없다(CPU, 메모리). 정보 줄만 보여 준다.
    Fixed,
    Gpu,
    Network,
    Disk,
}

impl Page {
    pub const ALL: [Page; 6] = [
        Page::General,
        Page::Cpu,
        Page::Memory,
        Page::Gpu,
        Page::Network,
        Page::Disk,
    ];

    /// 목록 위치에서 페이지로.
    pub fn from_index(index: usize) -> Option<Page> {
        Page::ALL.get(index).copied()
    }

    pub fn index(self) -> usize {
        Page::ALL.iter().position(|page| *page == self).unwrap_or(0)
    }

    /// 이 페이지에서 켜고 끄는 지표. `General`은 없다.
    pub fn metrics(self) -> &'static [Metric] {
        match self {
            Page::General => &[],
            Page::Cpu => &[Metric::Cpu],
            Page::Memory => &[Metric::Memory],
            Page::Gpu => &[Metric::Gpu, Metric::GpuVram],
            Page::Network => &[Metric::NetworkDown, Metric::NetworkUp],
            Page::Disk => &[Metric::DiskUsage, Metric::DiskActive],
        }
    }

    pub fn device(self) -> DeviceKind {
        match self {
            Page::General | Page::Cpu | Page::Memory => DeviceKind::Fixed,
            Page::Gpu => DeviceKind::Gpu,
            Page::Network => DeviceKind::Network,
            Page::Disk => DeviceKind::Disk,
        }
    }

    /// 목록의 색 표시와 선택 막대에 쓰는 지표 색. `General`은 테마 색을 쓴다.
    pub fn swatch(self) -> Option<Metric> {
        self.metrics().first().copied()
    }

    /// 목록과 페이지 제목.
    pub fn title(self) -> &'static str {
        let text = i18n::t();
        match self {
            Page::General => text.nav_general,
            Page::Cpu => text.nav_cpu,
            Page::Memory => text.nav_memory,
            Page::Gpu => text.nav_gpu,
            Page::Network => text.nav_network,
            Page::Disk => text.nav_disk,
        }
    }

    /// 제목 아래 한 줄 설명.
    pub fn hint(self) -> &'static str {
        let text = i18n::t();
        match self {
            Page::General => text.hint_page_general,
            Page::Cpu => text.hint_page_cpu,
            Page::Memory => text.hint_page_memory,
            Page::Gpu => text.hint_page_gpu,
            Page::Network => text.hint_page_network,
            Page::Disk => text.hint_page_disk,
        }
    }

    /// 장치 그룹의 설명.
    pub fn device_hint(self) -> &'static str {
        let text = i18n::t();
        match self {
            Page::General | Page::Cpu => text.hint_device_cpu,
            Page::Memory => text.hint_device_memory,
            Page::Gpu => text.hint_device_gpu,
            Page::Network => text.hint_device_network,
            Page::Disk => text.hint_device_disk,
        }
    }
}

/// 장치 그룹 안의 세로 배치(대화상자 단위). 콤보가 없는 페이지는 정보 줄이 올라온다.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DeviceLayout {
    /// 그룹 상자의 높이.
    pub group_height: i32,
    /// 정보 줄의 y.
    pub info_y: i32,
}

/// 장치 그룹 상자의 위쪽 y. `resources/app.rc`의 `IDC_GROUP_DEVICE`와 같아야 한다.
pub const DEVICE_GROUP_TOP: i32 = 85;

/// 그룹 안 격자(DLU). `resources/app.rc`의 주석과 같은 규칙이다.
pub mod grid {
    /// 그룹 위에서 설명 줄까지. 코드는 옮기지 않고 `.rc` 대조에만 쓴다.
    #[cfg(test)]
    pub const HINT: i32 = 11;
    /// 그룹 위에서 첫 줄까지.
    pub const FIRST_ROW: i32 = 24;
    /// 줄 간격.
    pub const ROW_PITCH: i32 = 14;
    /// 줄 하나의 높이(콤보·입력·버튼).
    pub const ROW_HEIGHT: i32 = 12;
    /// 마지막 줄 아래 여백.
    pub const BOTTOM: i32 = 7;
    /// 줄 위에서 글자 컨트롤까지. 글자(9)를 줄(12) 가운데에 맞춘다.
    pub const TEXT_OFFSET: i32 = 2;

    /// `row`번째(0부터) 줄의 위쪽. 그룹 위 기준.
    pub const fn row(row: i32) -> i32 {
        FIRST_ROW + ROW_PITCH * row
    }

    /// 줄이 `rows`개인 그룹의 높이.
    pub const fn group_height(rows: i32) -> i32 {
        row(rows - 1) + ROW_HEIGHT + BOTTOM
    }
}

/// 장치 그룹의 줄 수. 정보 줄도 한 줄로 센다.
pub const fn device_rows(kind: DeviceKind) -> i32 {
    match kind {
        // 정보 줄
        DeviceKind::Fixed => 1,
        // 콤보, 정보 줄
        DeviceKind::Gpu | DeviceKind::Disk => 2,
        // 콤보, 가상 어댑터, 정보 줄
        DeviceKind::Network => 3,
    }
}

/// 장치 그룹의 배치. 정보 줄은 늘 마지막 줄이다.
pub fn device_layout(kind: DeviceKind) -> DeviceLayout {
    let rows = device_rows(kind);
    DeviceLayout {
        group_height: grid::group_height(rows),
        info_y: DEVICE_GROUP_TOP + grid::row(rows - 1) + grid::TEXT_OFFSET,
    }
}

/// 장치 콤보의 항목 하나. `value`가 설정에 저장된다(빈 값은 자동).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Choice {
    pub label: String,
    pub value: String,
}

impl Choice {
    fn new(label: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            value: value.into(),
        }
    }
}

/// 콤보 항목과 지금 고른 위치.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Choices {
    pub items: Vec<Choice>,
    pub selected: usize,
}

/// 저장된 값을 고른다. 목록에 없으면(빼낸 장치) 표시를 붙여 뒤에 넣는다.
///
/// 없는 장치를 자동으로 바꿔 저장하지 않는다. 다시 꽂으면 그대로 따라가야 하기 때문이다.
fn finish(mut items: Vec<Choice>, saved: &str) -> Choices {
    let saved = saved.trim();
    if saved.is_empty() {
        return Choices { items, selected: 0 };
    }
    if let Some(index) = items.iter().position(|item| item.value == saved) {
        return Choices {
            items,
            selected: index,
        };
    }
    items.push(Choice::new(i18n::device_missing(saved), saved));
    let selected = items.len() - 1;
    Choices { items, selected }
}

/// GPU 콤보. `primary`는 자동일 때 고르는 어댑터의 이름이다.
pub fn gpu_choices(adapters: &[String], primary: Option<&str>, saved: &str) -> Choices {
    let auto = match primary {
        Some(name) => i18n::device_gpu_auto(name),
        None => i18n::device_gpu_auto("-"),
    };
    let mut items = vec![Choice::new(auto, "")];
    for name in adapters {
        // 같은 모델이 두 장이면 이름이 같다. 이름으로 저장하므로 하나만 둔다.
        if !items.iter().any(|item| item.value == *name) {
            items.push(Choice::new(name.clone(), name.clone()));
        }
    }
    finish(items, saved)
}

/// 네트워크 콤보. 연결되지 않은 인터페이스는 표시를 붙인다.
pub fn network_choices(interfaces: &[Interface], saved: &str) -> Choices {
    let mut items = vec![Choice::new(i18n::t().device_network_all, "")];
    for entry in interfaces {
        let mut label = if entry.description.is_empty() || entry.description == entry.alias {
            entry.alias.clone()
        } else {
            format!("{} · {}", entry.alias, entry.description)
        };
        if !entry.up {
            label = i18n::device_offline(&label);
        }
        items.push(Choice::new(label, entry.alias.clone()));
    }
    finish(items, saved)
}

/// 드라이브 이름. (`C: Local Disk`, `F: USB · removable`)
pub fn drive_label(drive: &Drive) -> String {
    let name = if drive.label.is_empty() {
        i18n::t().drive_local.to_string()
    } else {
        drive.label.clone()
    };
    let mut label = format!("{}: {name}", drive.letter);
    if drive.removable {
        label = format!("{label} · {}", i18n::t().drive_removable);
    }
    label
}

/// 디스크 콤보. 설정 값은 `D:` 꼴로 맞춰 비교한다.
pub fn disk_choices(drives: &[Drive], system: char, saved: &str) -> Choices {
    let mut items = vec![Choice::new(
        i18n::device_disk_auto(&disk::letter_value(system)),
        "",
    )];
    for drive in drives {
        items.push(Choice::new(
            drive_label(drive),
            disk::letter_value(drive.letter),
        ));
    }
    let saved = disk::parse_letter(saved)
        .map(disk::letter_value)
        .unwrap_or_default();
    finish(items, &saved)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::i18n::{Lang, set_language};

    /// 상태줄의 위쪽 y. 장치 그룹은 이 위에서 끝나야 한다(`IDC_STATUS`).
    const STATUS_TOP: i32 = 266;

    #[test]
    fn every_metric_lives_on_exactly_one_page() {
        for metric in Metric::ALL {
            let pages: Vec<Page> = Page::ALL
                .into_iter()
                .filter(|page| page.metrics().contains(&metric))
                .collect();
            assert_eq!(pages.len(), 1, "{metric:?}: {pages:?}");
        }
        assert!(Page::General.metrics().is_empty());
    }

    #[test]
    fn page_index_round_trips() {
        for page in Page::ALL {
            assert_eq!(Page::from_index(page.index()), Some(page));
        }
        assert_eq!(Page::from_index(Page::ALL.len()), None);
    }

    #[test]
    fn pages_have_text_in_every_language() {
        for lang in [Lang::En, Lang::Ko, Lang::Ja] {
            set_language(lang);
            for page in Page::ALL {
                assert!(!page.title().is_empty());
                assert!(!page.hint().is_empty());
                assert!(!page.device_hint().is_empty());
            }
        }
        set_language(Lang::En);
    }

    /// 장치 그룹은 상태줄 위에서 끝나고, 정보 줄은 그룹 안에 들어가야 한다.
    #[test]
    fn device_layouts_fit_above_the_status_line() {
        for kind in [
            DeviceKind::Fixed,
            DeviceKind::Gpu,
            DeviceKind::Network,
            DeviceKind::Disk,
        ] {
            let layout = device_layout(kind);
            assert!(
                DEVICE_GROUP_TOP + layout.group_height < STATUS_TOP,
                "{kind:?}"
            );
            assert!(layout.info_y + 9 < DEVICE_GROUP_TOP + layout.group_height);
        }
    }

    #[test]
    fn grid_heights_follow_the_row_count() {
        assert_eq!(grid::group_height(1), 43);
        assert_eq!(grid::group_height(2), 57);
        assert_eq!(grid::group_height(3), 71);
        let fixed = device_layout(DeviceKind::Fixed);
        assert_eq!(fixed.info_y, DEVICE_GROUP_TOP + 26);
        let network = device_layout(DeviceKind::Network);
        assert_eq!(network.group_height, 71);
        assert_eq!(network.info_y, DEVICE_GROUP_TOP + 54);
    }

    /// `.rc`의 그룹 위치와 이 파일의 상수가 어긋나면 정보 줄이 그룹 밖으로 나간다.
    #[test]
    fn layout_constants_match_the_resource_file() {
        let resource = include_str!("../../resources/app.rc");
        let top = |id: &str| -> i32 {
            let line = resource
                .lines()
                .find(|line| line.contains(&format!(", {id},")))
                .unwrap_or_else(|| panic!("{id}"));
            let numbers: Vec<i32> = line
                .split(',')
                .filter_map(|part| part.trim().parse().ok())
                .collect();
            numbers[1]
        };
        assert_eq!(top("IDC_GROUP_DEVICE"), DEVICE_GROUP_TOP);
        assert_eq!(top("IDC_STATUS"), STATUS_TOP);
    }

    #[test]
    fn gpu_choices_start_with_auto_and_keep_a_missing_adapter() {
        set_language(Lang::En);
        let adapters = vec!["iGPU".to_string(), "dGPU".to_string(), "dGPU".to_string()];

        let auto = gpu_choices(&adapters, Some("dGPU"), "");
        assert_eq!(auto.selected, 0);
        assert_eq!(auto.items[0].label, "Automatic (dGPU)");
        assert_eq!(auto.items[0].value, "");
        assert_eq!(auto.items.len(), 3, "같은 이름은 한 번만");

        let chosen = gpu_choices(&adapters, Some("dGPU"), "iGPU");
        assert_eq!(chosen.items[chosen.selected].value, "iGPU");

        let missing = gpu_choices(&adapters, Some("dGPU"), "eGPU");
        let entry = &missing.items[missing.selected];
        assert_eq!(entry.value, "eGPU", "없는 장치도 값을 지우지 않는다");
        assert_eq!(entry.label, "eGPU (not found)");
    }

    #[test]
    fn network_choices_name_adapters_by_alias() {
        set_language(Lang::En);
        let interfaces = vec![
            Interface {
                alias: "Ethernet".into(),
                description: "Intel(R) I211".into(),
                up: true,
            },
            Interface {
                alias: "Wi-Fi".into(),
                description: "Wi-Fi".into(),
                up: false,
            },
        ];
        let choices = network_choices(&interfaces, "Wi-Fi");
        assert_eq!(choices.items[0].label, "All adapters");
        assert_eq!(choices.items[1].label, "Ethernet · Intel(R) I211");
        assert_eq!(choices.items[2].label, "Wi-Fi (disconnected)");
        assert_eq!(choices.items[choices.selected].value, "Wi-Fi");
    }

    fn drive(letter: char, label: &str, removable: bool) -> Drive {
        Drive {
            letter,
            label: label.into(),
            removable,
            system: letter == 'C',
            total: 100,
            free: 40,
        }
    }

    #[test]
    fn disk_choices_normalise_the_saved_letter() {
        set_language(Lang::En);
        let drives = vec![drive('C', "", false), drive('F', "USB", true)];

        let choices = disk_choices(&drives, 'C', "f");
        assert_eq!(choices.items[0].label, "Automatic (system drive C:)");
        assert_eq!(choices.items[1].label, "C: Local Disk");
        assert_eq!(choices.items[2].label, "F: USB · removable");
        assert_eq!(choices.items[choices.selected].value, "F:");

        let gone = disk_choices(&drives, 'C', "Z:");
        assert_eq!(gone.items[gone.selected].label, "Z: (not found)");

        // 잘못된 값은 자동으로 본다.
        assert_eq!(disk_choices(&drives, 'C', "??").selected, 0);
    }

    #[test]
    fn choices_follow_the_language() {
        set_language(Lang::Ko);
        let choices = disk_choices(&[drive('C', "", false)], 'C', "");
        assert_eq!(choices.items[0].label, "자동 (시스템 드라이브 C:)");
        assert_eq!(choices.items[1].label, "C: 로컬 디스크");
        set_language(Lang::En);
    }
}
