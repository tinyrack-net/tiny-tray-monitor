//! 트레이 아이콘 슬롯과 우클릭 메뉴.
//!
//! `tray-icon`은 아이콘마다 숨은 창을 만들고 그 창에 메뉴를 붙인다. 따라서 아이콘 수만큼
//! [Menu]가 필요하다. muda의 아이템은 공유 가능하므로 같은 `CheckMenuItem`을 여러 메뉴에
//! 넣어 체크 상태를 하나로 유지한다.
//!
//! **아이콘은 재생성하지 않는다.** Windows는 아이콘 id를 생성 순서로 부여하고 트레이 승격
//! 설정이 그 (실행 경로, id)에 묶인다. 숨길 때는 `set_visible(false)`를 쓴다.
//!
//! 아이콘은 지표별 고정 GUID로 등록한다. Windows는 승격 설정을 GUID에 묶으므로,
//! 설치 경로가 바뀌는 MSIX 업데이트에서도 사용자가 고른 설정이 유지된다. GUID 값과
//! `Metric::ALL` 순서는 한 번 정해지면 바꾸지 않는다.

use tray_icon::menu::{
    CheckMenuItem, CheckMenuItemBuilder, IsMenuItem, Menu, MenuId, MenuItem, MenuItemBuilder,
    PredefinedMenuItem,
};
use tray_icon::{Icon, TrayIcon, TrayIconBuilder};

use crate::config::{ColorMode, Config};
use crate::icon::{self, GlyphCache, Metric};
use crate::logging;
use crate::metrics::format::Reading;
use crate::theme::Palette;
use crate::win32;

pub mod onboard;

/// 메뉴 항목 id. 이벤트 처리에서 문자열로 비교한다.
pub const MENU_SHOW: [&str; 4] = [
    "show-cpu",
    "show-memory",
    "show-network-down",
    "show-network-up",
];
pub const MENU_SETTINGS: &str = "settings";
pub const MENU_EXIT: &str = "exit";

/// 트레이 아이콘에 처음 등록할 때 쓰는 값. 첫 갱신 전까지 잠깐 보인다.
const PLACEHOLDER: &str = "0";

fn placeholder(metric: Metric) -> Reading {
    Reading {
        number: PLACEHOLDER.to_string(),
        unit: metric.base_unit(),
    }
}

/// 트레이 아이콘의 고정 GUID. `Metric::ALL` 순서와 같아야 하고, 버전이 바뀌어도
/// 절대 바꾸면 안 된다. Windows가 이 값으로 "작업 표시줄에 항상 표시" 설정을
/// 저장하므로, 실행 경로가 바뀌는 MSIX 업데이트에서도 승격 설정이 유지된다.
const TRAY_GUIDS: [u128; 4] = [
    0x08d913012075905f3b765d99cc00e07b, // CPU
    0x30146b445a48f8bc812370db292437b9, // MEM
    0x512275b8e98770860fd9c119653456c1, // DN
    0xecb03b17ed4a5b28b5f90f8642431ee3, // UP
];

/// 메뉴 아이템을 만들어 보관한다. 아이콘 수만큼 메뉴가 필요하므로 아이템을 공유한다.
pub struct MenuHandles {
    pub show: Vec<CheckMenuItem>,
    about: MenuItem,
    settings: MenuItem,
    exit: PredefinedMenuItem,
    separators: Vec<PredefinedMenuItem>,
}

impl MenuHandles {
    fn new(config: &Config) -> Self {
        let show = Metric::ALL
            .iter()
            .enumerate()
            .map(|(index, metric)| {
                CheckMenuItemBuilder::new()
                    .text(metric.tooltip_name())
                    .id(MenuId::new(MENU_SHOW[index]))
                    .checked(config.show.get(*metric))
                    .build()
            })
            .collect();

        Self {
            show,
            about: MenuItemBuilder::new()
                .text(format!(
                    "{} v{}",
                    crate::PRODUCT_NAME,
                    env!("CARGO_PKG_VERSION")
                ))
                .enabled(false)
                .build(),
            settings: MenuItemBuilder::new()
                .text("Settings…")
                .id(MenuId::new(MENU_SETTINGS))
                .build(),
            exit: PredefinedMenuItem::quit(Some("Exit")),
            separators: vec![
                PredefinedMenuItem::separator(),
                PredefinedMenuItem::separator(),
            ],
        }
    }

    /// 체크 표시를 실제 값에 맞춘다.
    ///
    /// 값이 바뀌는 경로는 하나(`App::apply_effects`)지만 표시는 창과 메뉴 두 곳이므로,
    /// 반영이 끝난 뒤 한 번에 맞춘다.
    pub fn sync(&self, config: &Config) {
        for (index, metric) in Metric::ALL.iter().enumerate() {
            self.show[index].set_checked(config.show.get(*metric));
        }
    }

    /// 같은 아이템을 공유하는 새 메뉴. 아이콘마다 하나씩 필요하다.
    fn build(&self) -> Result<Menu, String> {
        let items: Vec<&dyn IsMenuItem> = vec![
            &self.about,
            &self.separators[0],
            &self.show[0],
            &self.show[1],
            &self.show[2],
            &self.show[3],
            &self.separators[1],
            &self.settings,
            &self.exit,
        ];
        Menu::with_items(&items).map_err(|e| e.to_string())
    }
}

/// 지표 하나의 아이콘.
struct Slot {
    metric: Metric,
    tray: TrayIcon,
    /// 이 아이콘을 등록할 때 쓴 고정 GUID. 쓰지 않았거나 등록에 실패했으면 `None`.
    guid: Option<u128>,
    value: Reading,
    tooltip: String,
    dirty: bool,
}

pub struct Tray {
    px: i32,
    slots: Vec<Slot>,
    handles: MenuHandles,
    cache: GlyphCache,
    palette: Palette,
    color_mode: ColorMode,
    renders: u64,
}

impl Tray {
    /// 아이콘 4개를 [Metric::ALL] 순서로 만든다. 순서를 바꾸면 승격 설정이 어긋난다.
    pub fn new(px: i32, config: &Config, palette: Palette) -> Result<Self, String> {
        // 고정 GUID로 먼저 시도하고, 셸이 거부하면 GUID 없이 다시 만든다.
        match Self::build(px, config, palette, Some(TRAY_GUIDS)) {
            Ok(tray) => Ok(tray),
            Err(error) => {
                logging::warn(format!(
                    "could not register the tray icons with fixed GUIDs, retrying without GUIDs: {error}"
                ));
                Self::build(px, config, palette, None)
            }
        }
    }

    fn build(
        px: i32,
        config: &Config,
        palette: Palette,
        guids: Option<[u128; 4]>,
    ) -> Result<Self, String> {
        let handles = MenuHandles::new(config);
        let mut cache = GlyphCache::new(GlyphCache::DEFAULT_CAPACITY);
        let mut slots = Vec::with_capacity(Metric::ALL.len());

        for (index, metric) in Metric::ALL.into_iter().enumerate() {
            let menu = handles.build()?;
            let (label_color, value_color) = palette.colors(config.color_mode, metric);
            let bitmap = icon::stacked(
                &mut cache,
                &metric.caption(metric.base_unit()),
                PLACEHOLDER,
                px,
                label_color,
                value_color,
            );
            let tray_icon = Icon::from_rgba(bitmap.rgba, bitmap.width as u32, bitmap.height as u32)
                .map_err(|e| format!("cannot create the icon: {e}"))?;
            let mut builder = TrayIconBuilder::new()
                .with_icon(tray_icon)
                .with_tooltip(metric.tooltip_name())
                .with_menu(Box::new(menu));
            let guid = guids.map(|set| set[index]);
            if let Some(guid) = guid {
                builder = builder.with_guid(guid);
            }
            let tray = builder
                .build()
                .map_err(|e| format!("cannot register the tray icon: {e}"))?;

            // `tray-icon`은 셸이 GUID 등록을 거부해도 성공으로 돌려준다. 툴팁 갱신을
            // 한 번 보내 실제로 등록됐는지 확인하고, 실패하면 호출자가 GUID 없이
            // 다시 만들도록 오류를 돌려준다.
            if let Some(guid) = guid {
                if !win32::set_guid_tooltip(tray.window_handle(), guid, metric.tooltip_name()) {
                    return Err(format!(
                        "could not register the fixed GUID of the {metric:?} icon"
                    ));
                }
            }

            slots.push(Slot {
                metric,
                tray,
                guid,
                value: placeholder(metric),
                tooltip: metric.tooltip_name().to_string(),
                dirty: false,
            });
        }

        // `tray-icon`은 셸이 GUID를 거부해도 성공으로 돌려주고, NIM_MODIFY도 성공할
        // 수 있다. Explorer가 실제로 만든 `NotifyIconSettings` 항목을 확인해
        // GUID가 받아들여졌는지 판단한다.
        if let Some(guids) = guids {
            if !Self::wait_for_guid_entries(&guids) {
                return Err("the shell did not accept the fixed GUIDs".to_string());
            }
        }

        // 설정에서 꺼진 지표는 등록은 유지한 채 숨긴다.
        for slot in &mut slots {
            if !config.show.get(slot.metric) {
                if let Err(error) = slot.tray.set_visible(false) {
                    logging::warn(format!(
                        "could not hide the {:?} icon: {error}",
                        slot.metric
                    ));
                }
            }
        }

        Ok(Self {
            px,
            slots,
            handles,
            cache,
            palette,
            color_mode: config.color_mode,
            renders: 0,
        })
    }

    /// Explorer가 `NotifyIconSettings` 항목을 만들 때까지 잠깐 기다린다.
    fn wait_for_guid_entries(guids: &[u128; 4]) -> bool {
        for _ in 0..20 {
            if onboard::guids_present(guids) {
                return true;
            }
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
        false
    }

    fn slot(&mut self, metric: Metric) -> Option<&mut Slot> {
        self.slots.iter_mut().find(|slot| slot.metric == metric)
    }

    pub fn menu_handles(&self) -> &MenuHandles {
        &self.handles
    }

    /// 표시 여부를 바꾼다. 등록은 유지하므로 승격 설정이 보존된다.
    pub fn set_visible(&mut self, metric: Metric, visible: bool) {
        let Some(slot) = self.slot(metric) else {
            return;
        };
        if let Err(error) = slot.tray.set_visible(visible) {
            logging::warn(format!(
                "could not change the visibility of the {metric:?} icon: {error}"
            ));
        }
    }

    /// 테마나 색 모드가 바뀌면 다음 갱신에서 다시 그린다.
    pub fn set_style(&mut self, palette: Palette, color_mode: ColorMode) {
        if self.palette == palette && self.color_mode == color_mode {
            return;
        }
        self.palette = palette;
        self.color_mode = color_mode;
        for slot in &mut self.slots {
            slot.dirty = true;
        }
    }

    /// 값과 툴팁을 갱신한다. 표시 문자열이 그대로면 다시 그리지 않는다.
    pub fn update(&mut self, metric: Metric, value: Reading, tooltip: String) {
        let (palette, color_mode, px) = (self.palette, self.color_mode, self.px);
        let Some(slot) = self.slots.iter_mut().find(|slot| slot.metric == metric) else {
            return;
        };

        let tooltip_changed = slot.tooltip != tooltip;
        let value_changed = slot.value != value;
        if !value_changed && !tooltip_changed && !slot.dirty {
            return;
        }

        if value_changed || slot.dirty {
            let (label_color, value_color) = palette.colors(color_mode, metric);
            let bitmap = icon::stacked(
                &mut self.cache,
                &metric.caption(value.unit),
                &value.number,
                px,
                label_color,
                value_color,
            );
            match Icon::from_rgba(bitmap.rgba, bitmap.width as u32, bitmap.height as u32) {
                Ok(icon) => {
                    if let Err(error) = slot.tray.set_icon(Some(icon)) {
                        logging::warn(format!("could not update the {metric:?} icon: {error}"));
                    } else {
                        self.renders += 1;
                    }
                }
                Err(error) => {
                    logging::warn(format!("could not build the {metric:?} icon: {error}"))
                }
            }
            slot.value = value;
            slot.dirty = false;
        }

        if tooltip_changed {
            match slot.guid {
                Some(guid) => {
                    if !win32::set_guid_tooltip(slot.tray.window_handle(), guid, &tooltip) {
                        logging::warn(format!("could not update the {metric:?} tooltip"));
                    }
                }
                None => {
                    if let Err(error) = slot.tray.set_tooltip(Some(&tooltip)) {
                        logging::warn(format!("could not update the {metric:?} tooltip: {error}"));
                    }
                }
            }
            slot.tooltip = tooltip;
        }
    }

    pub fn cache_stats(&self) -> icon::cache::CacheStats {
        self.cache.stats()
    }

    pub fn render_count(&self) -> u64 {
        self.renders
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn menu_ids_are_unique() {
        let mut ids: Vec<&str> = MENU_SHOW.to_vec();
        ids.push(MENU_SETTINGS);
        ids.push(MENU_EXIT);
        let mut sorted = ids.clone();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(
            sorted.len(),
            ids.len(),
            "메뉴 id가 중복되면 이벤트가 섞인다"
        );
    }

    #[test]
    fn menu_can_be_built_for_every_icon() {
        let handles = MenuHandles::new(&Config::default());
        for _ in Metric::ALL {
            handles.build().expect("메뉴 생성");
        }
    }
}
