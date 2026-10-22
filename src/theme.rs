//! 작업 표시줄 테마에 맞춘 아이콘 색.
//!
//! 트레이 아이콘은 작업 표시줄 배경 위에 그려진다. 배경이 밝은데 흰 글자를 쓰면
//! 보이지 않으므로, Windows 모드(`SystemUsesLightTheme`)를 읽어 색을 뒤집는다.

use std::cell::Cell;

use crate::config::{ColorMode, UiTheme};
use crate::icon::{Color, Metric};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Palette {
    /// 라벨 색. 값보다 뒤로 물러난다.
    pub label: Color,
    /// 단색 모드에서 쓰는 전경색.
    pub mono: Color,
    pub cpu: Color,
    pub memory: Color,
    /// 수신(다운로드)
    pub network: Color,
    /// 송신(업로드). 수신과 구분되도록 다른 색을 쓴다.
    pub network_up: Color,
    pub gpu: Color,
    pub gpu_vram: Color,
    pub disk_usage: Color,
    pub disk_active: Color,
    pub light: bool,
}

impl Palette {
    /// 어두운 작업 표시줄용.
    pub const fn dark() -> Self {
        Self {
            label: Color(160, 160, 160),
            mono: Color(240, 240, 240),
            cpu: Color(110, 220, 110),
            memory: Color(110, 170, 255),
            network: Color(255, 180, 90),
            network_up: Color(190, 150, 255),
            gpu: Color(255, 130, 180),
            gpu_vram: Color(130, 230, 230),
            disk_usage: Color(240, 220, 100),
            disk_active: Color(255, 120, 100),
            light: false,
        }
    }

    /// 밝은 작업 표시줄용. 어두운 색을 써야 대비가 나온다.
    pub const fn light() -> Self {
        Self {
            label: Color(80, 80, 80),
            mono: Color(20, 20, 20),
            cpu: Color(20, 110, 20),
            memory: Color(20, 80, 190),
            network: Color(170, 75, 0),
            network_up: Color(95, 40, 165),
            gpu: Color(180, 20, 100),
            gpu_vram: Color(0, 130, 130),
            disk_usage: Color(130, 110, 0),
            disk_active: Color(185, 35, 20),
            light: true,
        }
    }

    pub const fn value(&self, metric: Metric) -> Color {
        match metric {
            Metric::Cpu => self.cpu,
            Metric::Memory => self.memory,
            Metric::NetworkDown => self.network,
            Metric::NetworkUp => self.network_up,
            Metric::Gpu => self.gpu,
            Metric::GpuVram => self.gpu_vram,
            Metric::DiskUsage => self.disk_usage,
            Metric::DiskActive => self.disk_active,
        }
    }

    /// 지표별 (라벨색, 값색).
    pub const fn colors(&self, mode: ColorMode, metric: Metric) -> (Color, Color) {
        match mode {
            ColorMode::Theme => (self.label, self.mono),
            ColorMode::Metric => (self.label, self.value(metric)),
        }
    }

    /// 작업 표시줄 배경색. 대비 테스트가 아이콘 색을 이 위에서 판정한다.
    #[cfg(test)]
    pub const fn taskbar(&self) -> Color {
        if self.light {
            Color(243, 243, 243)
        } else {
            Color(32, 32, 32)
        }
    }
}

const PERSONALIZE: &str = r"Software\Microsoft\Windows\CurrentVersion\Themes\Personalize";

/// 작업 표시줄이 밝은 모드인지. 읽지 못하면 어두운 모드(기본값)로 본다.
pub fn is_light() -> bool {
    windows_registry::CURRENT_USER
        .open(PERSONALIZE)
        .and_then(|key| key.get_u32("SystemUsesLightTheme"))
        .map(|value| value == 1)
        .unwrap_or(false)
}

thread_local! {
    /// 설정 창과 트레이 메뉴의 밝기. `App`이 설정에서 읽어 넣는다.
    static UI_THEME: Cell<UiTheme> = const { Cell::new(UiTheme::Auto) };
}

/// 창·메뉴 테마를 바꾼다. 아이콘 색([current])에는 영향이 없다.
pub fn set_ui_theme(theme: UiTheme) {
    UI_THEME.with(|cell| cell.set(theme));
}

/// 설정 창과 트레이 메뉴를 어둡게 그리는가.
///
/// 아이콘은 작업 표시줄 위에 놓이므로 늘 실제 배경을 따르지만, 창은 사용자가 고를 수 있다.
pub fn ui_dark() -> bool {
    resolve_dark(UI_THEME.with(Cell::get), is_light())
}

/// 고른 테마와 작업 표시줄 밝기로 창을 어둡게 그릴지 정한다.
pub fn resolve_dark(theme: UiTheme, taskbar_light: bool) -> bool {
    match theme {
        UiTheme::Auto => !taskbar_light,
        UiTheme::Light => false,
        UiTheme::Dark => true,
    }
}

/// 창과 같은 밝기의 색 묶음. 설정 창의 미리보기와 목록 색에 쓴다.
pub fn ui_palette() -> Palette {
    if ui_dark() {
        Palette::dark()
    } else {
        Palette::light()
    }
}

/// 현재 테마에 맞는 색 묶음.
pub fn current() -> Palette {
    if is_light() {
        Palette::light()
    } else {
        Palette::dark()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_metric_has_a_distinct_color() {
        for palette in [Palette::dark(), Palette::light()] {
            let colors = [
                palette.value(Metric::Cpu),
                palette.value(Metric::Memory),
                palette.value(Metric::NetworkDown),
                palette.value(Metric::NetworkUp),
                palette.value(Metric::Gpu),
                palette.value(Metric::GpuVram),
                palette.value(Metric::DiskUsage),
                palette.value(Metric::DiskActive),
            ];
            for (i, a) in colors.iter().enumerate() {
                for b in &colors[i + 1..] {
                    assert_ne!(a, b, "지표별 색이 겹치면 구분이 안 된다");
                }
                assert_ne!(*a, palette.label, "지표 색이 라벨 색과 겹친다");
            }
        }
    }

    /// 밝은 배경에서는 어두운 색, 어두운 배경에서는 밝은 색이어야 한다.
    ///
    /// 라벨은 알파로 물러나 있으므로 실제로 그려지는 색(배경과 합성한 값)으로 판정한다.
    #[test]
    fn colors_have_enough_contrast_against_the_taskbar() {
        let luminance =
            |c: Color| (0.2126 * c.0 as f32 + 0.7152 * c.1 as f32 + 0.0722 * c.2 as f32) / 255.0;
        let label_alpha = crate::icon::STACK.label_alpha as f32 / 255.0;

        for palette in [Palette::dark(), Palette::light()] {
            let background = palette.taskbar();
            let background_luminance = luminance(background);

            let cases = [
                ("label", palette.label, label_alpha),
                ("mono", palette.mono, 1.0),
                ("cpu", palette.value(Metric::Cpu), 1.0),
                ("memory", palette.value(Metric::Memory), 1.0),
                ("network down", palette.value(Metric::NetworkDown), 1.0),
                ("network up", palette.value(Metric::NetworkUp), 1.0),
                ("gpu", palette.value(Metric::Gpu), 1.0),
                ("gpu vram", palette.value(Metric::GpuVram), 1.0),
                ("disk usage", palette.value(Metric::DiskUsage), 1.0),
                ("disk active", palette.value(Metric::DiskActive), 1.0),
            ];
            for (name, color, alpha) in cases {
                let on_screen = luminance(color) * alpha + background_luminance * (1.0 - alpha);
                let contrast = (on_screen - background_luminance).abs();
                assert!(
                    contrast > 0.2,
                    "{name} {color:?} on {} background: contrast {contrast:.2}",
                    if palette.light { "light" } else { "dark" }
                );
            }
        }
    }

    #[test]
    fn current_palette_is_readable() {
        let palette = current();
        assert_eq!(palette.light, is_light());
    }

    #[test]
    fn colors_follow_the_mode() {
        let palette = Palette::dark();
        let (label, value) = palette.colors(ColorMode::Theme, Metric::Cpu);
        assert_eq!(label, palette.label);
        assert_eq!(value, palette.mono);

        let (_, value) = palette.colors(ColorMode::Metric, Metric::Cpu);
        assert_eq!(value, palette.cpu);
    }

    #[test]
    fn metric_colors_differ_by_metric() {
        let palette = Palette::dark();
        let color = |metric| palette.colors(ColorMode::Metric, metric).1;
        assert_ne!(color(Metric::Cpu), color(Metric::Memory));
        assert_ne!(color(Metric::Memory), color(Metric::NetworkDown));
        assert_ne!(color(Metric::NetworkDown), color(Metric::NetworkUp));
    }

    #[test]
    fn ui_theme_overrides_the_taskbar() {
        for taskbar_light in [true, false] {
            assert_eq!(resolve_dark(UiTheme::Auto, taskbar_light), !taskbar_light);
            assert!(!resolve_dark(UiTheme::Light, taskbar_light));
            assert!(resolve_dark(UiTheme::Dark, taskbar_light));
        }
    }

    /// 미리보기 바탕은 아이콘이 실제로 놓이는 배경과 같아야 한다.
    #[test]
    fn taskbar_background_matches_the_contrast_check() {
        assert_eq!(Palette::light().taskbar(), Color(243, 243, 243));
        assert_eq!(Palette::dark().taskbar(), Color(32, 32, 32));
    }
}
