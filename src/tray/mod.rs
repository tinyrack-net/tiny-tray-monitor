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
use crate::i18n;
use crate::icon::{self, GlyphCache, Metric};
use crate::logging;
use crate::metrics::format::Reading;
use crate::theme::Palette;
use crate::win32;

/// 메뉴 항목 id. 이벤트 처리에서 문자열로 비교한다.
pub const MENU_SHOW: [&str; 8] = [
    "show-cpu",
    "show-memory",
    "show-network-down",
    "show-network-up",
    "show-gpu",
    "show-gpu-vram",
    "show-disk-usage",
    "show-disk-active",
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
const TRAY_GUIDS: [u128; 8] = [
    0x08d913012075905f3b765d99cc00e07b, // CPU
    0x30146b445a48f8bc812370db292437b9, // MEM
    0x512275b8e98770860fd9c119653456c1, // DN
    0xecb03b17ed4a5b28b5f90f8642431ee3, // UP
    0x451ac7457b5f28feb60d30f69e64b770, // GPU. v0.7.0에서 새로 발급했다.
    0x5a61af487507da6c2d7b7302fd3e2479, // VRM. v0.7.0에서 새로 발급했다.
    0x628942a3611444deb862bea6c14d26f3, // DSK. v0.11.0에서 새로 발급했다.
    0x879173999413413caa3b8a9afe792083, // DIO. v0.11.0에서 새로 발급했다.
];

/// 지표를 추가하면서 GUID를 빠뜨리면 여기서 컴파일이 멈춘다.
const _: () = assert!(TRAY_GUIDS.len() == Metric::ALL.len());

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
                    .text(i18n::metric_label(*metric))
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
                .text(i18n::t().menu_settings)
                .id(MenuId::new(MENU_SETTINGS))
                .build(),
            exit: PredefinedMenuItem::quit(Some(i18n::t().menu_exit)),
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

    /// 언어가 바뀌었을 때 메뉴 글자를 다시 입힌다.
    ///
    /// 트레이 아이콘은 재생성하지 않는다(승격 설정과 아이콘 id 보존). 메뉴 텍스트만 바꾼다.
    pub fn relabel(&self) {
        for (index, metric) in Metric::ALL.iter().enumerate() {
            self.show[index].set_text(i18n::metric_label(*metric));
        }
        self.settings.set_text(i18n::t().menu_settings);
        self.exit.set_text(i18n::t().menu_exit);
    }

    /// 같은 아이템을 공유하는 새 메뉴. 아이콘마다 하나씩 필요하다.
    fn build(&self) -> Result<Menu, String> {
        // 항목 수를 손으로 세지 않는다. 지표를 추가하면 메뉴에도 저절로 붙는다.
        let mut items: Vec<&dyn IsMenuItem> = vec![&self.about, &self.separators[0]];
        for item in &self.show {
            items.push(item);
        }
        items.push(&self.separators[1]);
        items.push(&self.settings);
        items.push(&self.exit);
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
    /// 아이콘을 [Metric::ALL] 순서로 전부 만든다. 순서를 바꾸면 승격 설정이 어긋난다.
    ///
    /// **고정 GUID로 등록하는 것이 기본이다.** 아이콘마다 툴팁을 `NIF_GUID`로 한 번
    /// 되쓰는 것으로 셸이 그 GUID의 아이콘을 들고 있는지 확인하고, 실패한 아이콘이
    /// 있으면 GUID 없이 다시 만든다.
    ///
    /// `NotifyIconSettings`에 키가 없다는 것은 폴백 사유가 아니다. 셸은 이미 아는
    /// 정체성에는 키를 다시 쓰지 않으므로, 그걸 거부로 읽으면 실행할 때마다 폴백하고
    /// 정체성이 (경로, uID)로 굳어 승격 설정이 지표 추가/경로 변경마다 어긋난다
    /// (실측: docs/SPIKE.md §15).
    pub fn new(px: i32, config: &Config, palette: Palette) -> Result<Self, String> {
        match Self::build(px, config, palette, Some(&TRAY_GUIDS)) {
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
        guids: Option<&[u128]>,
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
                .with_tooltip(i18n::metric_label(metric))
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
                if !win32::set_guid_tooltip(tray.window_handle(), guid, i18n::metric_label(metric))
                {
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
                tooltip: i18n::metric_label(metric).to_string(),
            });
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

    fn slot(&mut self, metric: Metric) -> Option<&mut Slot> {
        self.slots.iter_mut().find(|slot| slot.metric == metric)
    }

    pub fn menu_handles(&self) -> &MenuHandles {
        &self.handles
    }

    /// 언어가 바뀌면 메뉴 글자를 새 언어로 맞춘다.
    ///
    /// 툴팁은 다음 틱이 새 언어로 만들어 [`Tray::update`]의 문자열 비교에 걸려 갱신된다.
    pub fn relabel(&mut self) {
        self.handles.relabel();
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

    /// 아이콘 크기(px)를 바꾼다. 배율이 바뀌면 부른다.
    ///
    /// 아이콘은 다시 만들지 않고 `set_icon`으로 새 크기의 비트맵만 넘긴다. 등록(GUID, uID)은
    /// 그대로라 승격 설정이 보존된다. 값이 오지 않는 지표(GPU가 없는 기기 등)도 옛 크기로
    /// 남지 않도록 다음 갱신을 기다리지 않고 지금 전부 다시 그린다. 바뀌었으면 이전 크기를
    /// 돌려준다.
    pub fn set_px(&mut self, px: i32) -> Option<i32> {
        let previous = resize(&mut self.px, px)?;
        self.redraw_all();
        Some(previous)
    }

    /// 테마나 색 모드가 바뀌면 지금 전부 다시 그린다.
    pub fn set_style(&mut self, palette: Palette, color_mode: ColorMode) {
        if self.palette == palette && self.color_mode == color_mode {
            return;
        }
        self.palette = palette;
        self.color_mode = color_mode;
        self.redraw_all();
    }

    fn redraw_all(&mut self) {
        let style = (self.palette, self.color_mode, self.px);
        for slot in &mut self.slots {
            if draw(&mut self.cache, slot, style) {
                self.renders += 1;
            }
        }
    }

    /// 값과 툴팁을 갱신한다. 표시 문자열이 그대로면 다시 그리지 않는다.
    pub fn update(&mut self, metric: Metric, value: Reading, tooltip: String) {
        let style = (self.palette, self.color_mode, self.px);
        let Some(slot) = self.slots.iter_mut().find(|slot| slot.metric == metric) else {
            return;
        };

        let tooltip_changed = slot.tooltip != tooltip;
        let value_changed = slot.value != value;
        if !value_changed && !tooltip_changed {
            return;
        }

        if value_changed {
            slot.value = value;
            if draw(&mut self.cache, slot, style) {
                self.renders += 1;
            }
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

/// 슬롯의 현재 값을 `style`(팔레트, 색 모드, px)로 그려 아이콘을 바꾼다. 바꿨으면 `true`.
fn draw(cache: &mut GlyphCache, slot: &Slot, style: (Palette, ColorMode, i32)) -> bool {
    let (palette, color_mode, px) = style;
    let metric = slot.metric;
    let (label_color, value_color) = palette.colors(color_mode, metric);
    let bitmap = icon::stacked(
        cache,
        &metric.caption(slot.value.unit),
        &slot.value.number,
        px,
        label_color,
        value_color,
    );
    match Icon::from_rgba(bitmap.rgba, bitmap.width as u32, bitmap.height as u32) {
        Ok(icon) => match slot.tray.set_icon(Some(icon)) {
            Ok(()) => true,
            Err(error) => {
                logging::warn(format!("could not update the {metric:?} icon: {error}"));
                false
            }
        },
        Err(error) => {
            logging::warn(format!("could not build the {metric:?} icon: {error}"));
            false
        }
    }
}

/// `current`를 `next`로 바꾼다. 같거나 잘못된 값이면 그대로 두고 `None`, 바뀌면 이전 값.
fn resize(current: &mut i32, next: i32) -> Option<i32> {
    if next <= 0 || *current == next {
        return None;
    }
    Some(std::mem::replace(current, next))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resize_reports_only_real_changes() {
        let mut px = 16;
        assert_eq!(resize(&mut px, 16), None, "같은 크기면 다시 그리지 않는다");
        assert_eq!(resize(&mut px, 36), Some(16));
        assert_eq!(px, 36);
        assert_eq!(resize(&mut px, 0), None, "잘못된 크기는 무시한다");
        assert_eq!(px, 36);
        assert_eq!(resize(&mut px, 16), Some(36));
    }

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
