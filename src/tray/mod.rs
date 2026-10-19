//! 트레이 아이콘 슬롯과 우클릭 메뉴.
//!
//! `tray-icon`은 아이콘마다 숨은 창을 만들고 그 창에 메뉴를 붙인다. 따라서 아이콘 수만큼
//! [Menu]가 필요하다. muda의 아이템은 공유 가능하므로 같은 `CheckMenuItem`을 여러 메뉴에
//! 넣어 체크 상태를 하나로 유지한다.
//!
//! **아이콘은 재생성하지 않는다.** Windows는 아이콘 id를 생성 순서로 부여하고 트레이 승격
//! 설정이 그 (실행 경로, id)에 묶인다. 숨길 때는 `set_visible(false)`를 쓴다.

use tray_icon::menu::{
    CheckMenuItem, CheckMenuItemBuilder, IsMenuItem, Menu, MenuId, MenuItem, MenuItemBuilder,
    PredefinedMenuItem,
};
use tray_icon::{Icon, TrayIcon, TrayIconBuilder};

use crate::config::{ColorMode, Config};
use crate::icon::{self, GlyphCache, Metric, StackSpec};
use crate::logging;
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
pub const MENU_AUTOSTART: &str = "autostart";
pub const MENU_VIRTUAL: &str = "count-virtual-adapters";
pub const MENU_SETTINGS: &str = "settings";
pub const MENU_OPEN_CONFIG: &str = "open-config";
pub const MENU_EXIT: &str = "exit";

/// 트레이 아이콘에 처음 등록할 때 쓰는 값. 첫 갱신 전까지 잠깐 보인다.
const PLACEHOLDER: &str = "0";

/// 메뉴 아이템을 만들어 보관한다. 아이콘 수만큼 메뉴가 필요하므로 아이템을 공유한다.
pub struct MenuHandles {
    pub show: Vec<CheckMenuItem>,
    pub autostart: CheckMenuItem,
    pub virtual_adapters: CheckMenuItem,
    about: MenuItem,
    settings: MenuItem,
    open_config: MenuItem,
    exit: PredefinedMenuItem,
    separators: Vec<PredefinedMenuItem>,
}

impl MenuHandles {
    fn new(config: &Config, autostart: bool) -> Self {
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
            autostart: CheckMenuItemBuilder::new()
                .text("Run at sign-in")
                .id(MenuId::new(MENU_AUTOSTART))
                .checked(autostart)
                .build(),
            // 가상 어댑터를 빼면 물리 NIC와 가상 스위치가 같은 트래픽을 두 번 세지 않는다.
            // VPN만 쓰는 환경에서는 오히려 값이 0이 되므로 토글로 열어 둔다.
            virtual_adapters: CheckMenuItemBuilder::new()
                .text("Count virtual adapters")
                .id(MenuId::new(MENU_VIRTUAL))
                .checked(!config.exclude_virtual_adapters)
                .build(),
            about: MenuItemBuilder::new()
                .text(format!("tray-monitor v{}", env!("CARGO_PKG_VERSION")))
                .enabled(false)
                .build(),
            open_config: MenuItemBuilder::new()
                .text("Open config file")
                .id(MenuId::new(MENU_OPEN_CONFIG))
                .build(),
            settings: MenuItemBuilder::new()
                .text("Settings…")
                .id(MenuId::new(MENU_SETTINGS))
                .build(),
            exit: PredefinedMenuItem::quit(Some("Exit")),
            separators: vec![
                PredefinedMenuItem::separator(),
                PredefinedMenuItem::separator(),
                PredefinedMenuItem::separator(),
            ],
        }
    }

    /// 체크 표시를 실제 값에 맞춘다.
    ///
    /// 값이 바뀌는 경로는 하나(`App::apply_effects`)지만 표시는 창과 메뉴 두 곳이므로,
    /// 반영이 끝난 뒤 한 번에 맞춘다.
    pub fn sync(&self, config: &Config, autostart: bool) {
        for (index, metric) in Metric::ALL.iter().enumerate() {
            self.show[index].set_checked(config.show.get(*metric));
        }
        self.autostart.set_checked(autostart);
        self.virtual_adapters
            .set_checked(!config.exclude_virtual_adapters);
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
            &self.autostart,
            &self.virtual_adapters,
            &self.separators[2],
            &self.settings,
            &self.open_config,
            &self.exit,
        ];
        Menu::with_items(&items).map_err(|e| e.to_string())
    }
}

/// 지표 하나의 아이콘.
struct Slot {
    metric: Metric,
    tray: TrayIcon,
    value: String,
    tooltip: String,
    dirty: bool,
}

pub struct Tray {
    px: i32,
    spec: StackSpec,
    slots: Vec<Slot>,
    handles: MenuHandles,
    cache: GlyphCache,
    palette: Palette,
    color_mode: ColorMode,
    renders: u64,
}

impl Tray {
    /// 아이콘 3개를 [Metric::ALL] 순서로 만든다. 순서를 바꾸면 승격 설정이 어긋난다.
    pub fn new(
        px: i32,
        config: &Config,
        palette: Palette,
        autostart: bool,
    ) -> Result<Self, String> {
        let spec = icon::stack_spec(px);
        let handles = MenuHandles::new(config, autostart);
        let mut cache = GlyphCache::new(GlyphCache::DEFAULT_CAPACITY);
        let mut slots = Vec::with_capacity(Metric::ALL.len());

        for metric in Metric::ALL {
            let menu = handles.build()?;
            let (label_color, value_color) = palette.colors(config.color_mode, metric);
            let bitmap = icon::stacked(
                &mut cache,
                metric.label(),
                PLACEHOLDER,
                px,
                spec,
                label_color,
                value_color,
            );
            let tray_icon = Icon::from_rgba(bitmap.rgba, bitmap.width as u32, bitmap.height as u32)
                .map_err(|e| format!("아이콘을 만들 수 없다: {e}"))?;
            let tray = TrayIconBuilder::new()
                .with_icon(tray_icon)
                .with_tooltip(metric.tooltip_name())
                .with_menu(Box::new(menu))
                .build()
                .map_err(|e| format!("트레이 아이콘을 등록할 수 없다: {e}"))?;

            slots.push(Slot {
                metric,
                tray,
                value: PLACEHOLDER.to_string(),
                tooltip: metric.tooltip_name().to_string(),
                dirty: false,
            });
        }

        // 설정에서 꺼진 지표는 등록은 유지한 채 숨긴다.
        for slot in &mut slots {
            if !config.show.get(slot.metric) {
                if let Err(error) = slot.tray.set_visible(false) {
                    logging::warn(format!("{:?} 아이콘을 숨기지 못했다: {error}", slot.metric));
                }
            }
        }

        Ok(Self {
            px,
            spec,
            slots,
            handles,
            cache,
            palette,
            color_mode: config.color_mode,
            renders: 0,
        })
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
            logging::warn(format!("{metric:?} 아이콘 표시를 바꾸지 못했다: {error}"));
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
    pub fn update(&mut self, metric: Metric, value: String, tooltip: String) {
        let (palette, color_mode, px, spec) = (self.palette, self.color_mode, self.px, self.spec);
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
                metric.label(),
                &value,
                px,
                spec,
                label_color,
                value_color,
            );
            match Icon::from_rgba(bitmap.rgba, bitmap.width as u32, bitmap.height as u32) {
                Ok(icon) => {
                    if let Err(error) = slot.tray.set_icon(Some(icon)) {
                        logging::warn(format!("{metric:?} 아이콘을 바꾸지 못했다: {error}"));
                    } else {
                        self.renders += 1;
                    }
                }
                Err(error) => logging::warn(format!("{metric:?} 아이콘 생성 실패: {error}")),
            }
            slot.value = value;
            slot.dirty = false;
        }

        if tooltip_changed {
            if let Err(error) = slot.tray.set_tooltip(Some(&tooltip)) {
                logging::warn(format!("{metric:?} 툴팁을 바꾸지 못했다: {error}"));
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

/// 설정 파일을 기본 편집기로 연다.
pub fn open_config_file() {
    let path = crate::config::config_dir().join("config.toml");
    if !path.exists() {
        let _ = std::fs::create_dir_all(crate::config::config_dir());
        let _ = std::fs::write(&path, "");
    }
    if !win32::open_url(&path.display().to_string()) {
        logging::warn(format!("{} 를 열지 못했다", path.display()));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn menu_ids_are_unique() {
        let mut ids: Vec<&str> = MENU_SHOW.to_vec();
        ids.push(MENU_AUTOSTART);
        ids.push(MENU_VIRTUAL);
        ids.push(MENU_SETTINGS);
        ids.push(MENU_OPEN_CONFIG);
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
        let handles = MenuHandles::new(&Config::default(), false);
        for _ in Metric::ALL {
            handles.build().expect("메뉴 생성");
        }
    }
}
