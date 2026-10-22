//! 설정 창과 트레이 메뉴가 함께 쓰는 편집 상태와 적용 계획.
//!
//! 화면(대화상자)과 저장 구조([Config]) 사이를 순수 함수로 잇는다. 여기 있는 것은 부작용이
//! 없어서 전부 단위 테스트로 검증한다: 값 보정, 마지막 지표 규칙, 바뀐 항목만 뽑아내기.

use crate::config::{
    ColorMode, Config, Devices, MAX_POLL_INTERVAL_MS, MIN_POLL_INTERVAL_MS, Show, UiTheme,
};
use crate::i18n::Language;
use crate::icon::Metric;

/// 콤보에 넣는 폴링 주기(ms).
pub const INTERVAL_PRESETS: [u64; 6] = [250, 500, 1_000, 2_000, 5_000, 10_000];

/// 편집 중인 값. 저장하기 전 상태이므로 [Config]와 분리해 둔다.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SettingsDraft {
    pub poll_interval_ms: u64,
    pub show: Show,
    pub color_mode: ColorMode,
    pub ui_theme: UiTheme,
    /// 가상 어댑터를 집계에 넣는가. `Config`에는 반대 의미로 저장된다.
    pub count_virtual: bool,
    pub autostart: bool,
    pub language: Language,
    /// 분야별 장치. 빈 값은 자동이다.
    pub devices: Devices,
}

/// [SettingsDraft::validate]가 값을 바로잡을 때 남기는 메모.
///
/// 문자열이 아니라 값으로 돌려준다. 같은 메모를 로그에는 영문으로, 상태줄에는 지금
/// 언어로 보여 줘야 하기 때문이다.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Note {
    /// 주기를 허용 범위로 잘랐다.
    IntervalClamped { from: u64, to: u64 },
    /// 표시 항목이 하나도 없어 CPU를 켰다.
    NoReadoutEnabled,
}

impl SettingsDraft {
    pub fn from_config(config: &Config, autostart: bool) -> Self {
        Self {
            poll_interval_ms: config.poll_interval_ms,
            show: config.show,
            color_mode: config.color_mode,
            ui_theme: config.ui_theme,
            count_virtual: !config.exclude_virtual_adapters,
            autostart,
            language: config.language,
            devices: config.devices.clone(),
        }
    }

    /// 범위를 벗어난 값을 바로잡고, 바꾼 항목을 돌려준다.
    pub fn validate(&mut self) -> Vec<Note> {
        let mut notes = Vec::new();

        let clamped = self
            .poll_interval_ms
            .clamp(MIN_POLL_INTERVAL_MS, MAX_POLL_INTERVAL_MS);
        if clamped != self.poll_interval_ms {
            notes.push(Note::IntervalClamped {
                from: self.poll_interval_ms,
                to: clamped,
            });
            self.poll_interval_ms = clamped;
        }

        if self.show.count() == 0 {
            notes.push(Note::NoReadoutEnabled);
            self.show.cpu = true;
        }

        notes
    }
}

/// 적용해야 할 변경 하나.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Effect {
    Interval(u64),
    Visible(Metric, bool),
    ColorMode(ColorMode),
    UiTheme(UiTheme),
    CountVirtual(bool),
    Autostart(bool),
    Language(Language),
    /// GPU 어댑터 이름. 빈 값은 자동.
    GpuDevice(String),
    /// 네트워크 인터페이스 이름. 빈 값은 모든 어댑터.
    NetworkDevice(String),
    /// 드라이브(`D:`). 빈 값은 시스템 드라이브.
    DiskDevice(String),
}

impl Effect {
    /// [Config]에 반영한다. 순수하므로 그대로 테스트한다.
    ///
    /// 자동 실행은 설정 파일이 아니라 레지스트리가 원본이라 [Config]에는 아무것도 하지 않는다.
    pub fn apply_to_config(&self, config: &mut Config) {
        match self {
            Effect::Interval(ms) => config.poll_interval_ms = *ms,
            Effect::Visible(metric, visible) => config.show.set(*metric, *visible),
            Effect::ColorMode(mode) => config.color_mode = *mode,
            Effect::UiTheme(theme) => config.ui_theme = *theme,
            Effect::CountVirtual(count) => config.exclude_virtual_adapters = !*count,
            Effect::Language(language) => config.language = *language,
            Effect::GpuDevice(name) => config.devices.gpu = name.clone(),
            Effect::NetworkDevice(name) => config.devices.network = name.clone(),
            Effect::DiskDevice(drive) => config.devices.disk = drive.clone(),
            Effect::Autostart(_) => {}
        }
    }
}

/// 마지막으로 남은 지표를 끄려 했다.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LastReadout;

/// 지표 표시를 바꾼다. 마지막 남은 하나는 끌 수 없다.
///
/// 전부 끄면 트레이에서 아이콘이 사라져 되돌릴 방법이 없다. 설정 창과 트레이 메뉴가 같은
/// 규칙을 쓰도록 여기에 둔다. 문구는 [crate::i18n::t]의 `last_readout_error`를 쓴다.
pub fn set_visible(show: &mut Show, metric: Metric, visible: bool) -> Result<(), LastReadout> {
    if !visible && show.get(metric) && show.count() <= 1 {
        return Err(LastReadout);
    }
    show.set(metric, visible);
    Ok(())
}

/// 이미 적용된 값과 편집 중인 값의 차이만 뽑는다.
pub fn diff(applied: &SettingsDraft, next: &SettingsDraft) -> Vec<Effect> {
    let mut effects = Vec::new();

    if applied.poll_interval_ms != next.poll_interval_ms {
        effects.push(Effect::Interval(next.poll_interval_ms));
    }

    for metric in Metric::ALL {
        let before = applied.show.get(metric);
        let after = next.show.get(metric);
        if before != after {
            effects.push(Effect::Visible(metric, after));
        }
    }

    if applied.color_mode != next.color_mode {
        effects.push(Effect::ColorMode(next.color_mode));
    }

    if applied.ui_theme != next.ui_theme {
        effects.push(Effect::UiTheme(next.ui_theme));
    }

    if applied.count_virtual != next.count_virtual {
        effects.push(Effect::CountVirtual(next.count_virtual));
    }

    if applied.autostart != next.autostart {
        effects.push(Effect::Autostart(next.autostart));
    }

    if applied.language != next.language {
        effects.push(Effect::Language(next.language));
    }

    if applied.devices.gpu != next.devices.gpu {
        effects.push(Effect::GpuDevice(next.devices.gpu.clone()));
    }

    if applied.devices.network != next.devices.network {
        effects.push(Effect::NetworkDevice(next.devices.network.clone()));
    }

    if applied.devices.disk != next.devices.disk {
        effects.push(Effect::DiskDevice(next.devices.disk.clone()));
    }

    effects
}

/// 프리셋에 있는 값인가. 아니면 콤보에서 `Custom`을 골라야 한다.
pub fn is_preset(ms: u64) -> bool {
    INTERVAL_PRESETS.contains(&ms)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn draft(interval: u64) -> SettingsDraft {
        SettingsDraft::from_config(&Config::default(), false).with_interval(interval)
    }

    impl SettingsDraft {
        fn with_interval(mut self, ms: u64) -> Self {
            self.poll_interval_ms = ms;
            self
        }
    }

    #[test]
    fn defaults_are_valid_and_change_nothing() {
        let applied = SettingsDraft::from_config(&Config::default(), false);
        let mut next = applied.clone();
        assert!(next.validate().is_empty());
        assert!(diff(&applied, &next).is_empty());

        next.poll_interval_ms += 1;
        assert_eq!(diff(&applied, &next), vec![Effect::Interval(1_001)]);
    }

    #[test]
    fn validate_clamps_the_interval() {
        let mut too_fast = draft(1);
        assert_eq!(
            too_fast.validate(),
            vec![Note::IntervalClamped {
                from: 1,
                to: MIN_POLL_INTERVAL_MS
            }]
        );
        assert_eq!(too_fast.poll_interval_ms, MIN_POLL_INTERVAL_MS);

        let mut too_slow = draft(60_000);
        assert_eq!(
            too_slow.validate(),
            vec![Note::IntervalClamped {
                from: 60_000,
                to: MAX_POLL_INTERVAL_MS
            }]
        );
        assert_eq!(too_slow.poll_interval_ms, MAX_POLL_INTERVAL_MS);

        let mut fine = draft(MAX_POLL_INTERVAL_MS);
        assert!(fine.validate().is_empty());
    }

    #[test]
    fn validate_keeps_at_least_one_metric() {
        let mut empty = draft(1_000);
        for metric in Metric::ALL {
            empty.show.set(metric, false);
        }
        assert_eq!(empty.validate(), vec![Note::NoReadoutEnabled]);
        assert!(empty.show.cpu, "전부 꺼지면 아이콘이 사라진다");
    }

    /// 마지막 하나는 끌 수 없고, 둘 이상이면 끌 수 있다.
    #[test]
    fn set_visible_refuses_the_last_readout() {
        let mut show = Show::default();
        for metric in Metric::ALL.iter().skip(1) {
            assert!(set_visible(&mut show, *metric, false).is_ok());
        }
        assert_eq!(show.count(), 1);

        let error = set_visible(&mut show, Metric::Cpu, false).expect_err("마지막은 못 끈다");
        assert_eq!(error, LastReadout);
        assert!(show.cpu, "실패하면 값을 바꾸지 않는다");

        // 이미 꺼져 있는 지표를 다시 끄는 것은 막을 이유가 없다.
        assert!(set_visible(&mut show, Metric::Memory, false).is_ok());

        assert!(set_visible(&mut show, Metric::Memory, true).is_ok());
        assert!(set_visible(&mut show, Metric::Cpu, false).is_ok());
    }

    #[test]
    fn diff_reports_only_what_changed() {
        let applied = SettingsDraft::from_config(&Config::default(), false);

        let mut one = applied.clone();
        one.show.set(Metric::NetworkUp, false);
        assert_eq!(
            diff(&applied, &one),
            vec![Effect::Visible(Metric::NetworkUp, false)]
        );

        let mut color = applied.clone();
        color.color_mode = ColorMode::Theme;
        assert_eq!(
            diff(&applied, &color),
            vec![Effect::ColorMode(ColorMode::Theme)]
        );

        let mut ui_theme = applied.clone();
        ui_theme.ui_theme = UiTheme::Light;
        assert_eq!(
            diff(&applied, &ui_theme),
            vec![Effect::UiTheme(UiTheme::Light)]
        );

        let mut virtual_adapters = applied.clone();
        virtual_adapters.count_virtual = true;
        assert_eq!(
            diff(&applied, &virtual_adapters),
            vec![Effect::CountVirtual(true)]
        );

        let mut autostart = applied.clone();
        autostart.autostart = true;
        assert_eq!(diff(&applied, &autostart), vec![Effect::Autostart(true)]);

        let mut language = applied.clone();
        language.language = Language::Ja;
        assert_eq!(
            diff(&applied, &language),
            vec![Effect::Language(Language::Ja)]
        );
    }

    /// 여러 항목이 바뀌면 지표 순서대로 한 번에 나온다.
    #[test]
    fn diff_collects_every_change() {
        let applied = SettingsDraft::from_config(&Config::default(), false);
        let mut next = applied.clone();
        next.poll_interval_ms = 500;
        next.show.cpu = false;
        next.show.memory = false;
        next.count_virtual = true;

        assert_eq!(
            diff(&applied, &next),
            vec![
                Effect::Interval(500),
                Effect::Visible(Metric::Cpu, false),
                Effect::Visible(Metric::Memory, false),
                Effect::CountVirtual(true),
            ]
        );
    }

    #[test]
    fn effects_land_in_the_config() {
        let mut config = Config::default();

        Effect::Interval(500).apply_to_config(&mut config);
        Effect::Visible(Metric::NetworkDown, false).apply_to_config(&mut config);
        Effect::ColorMode(ColorMode::Theme).apply_to_config(&mut config);
        Effect::UiTheme(UiTheme::Dark).apply_to_config(&mut config);
        Effect::CountVirtual(true).apply_to_config(&mut config);
        Effect::Language(Language::Ko).apply_to_config(&mut config);
        Effect::Autostart(true).apply_to_config(&mut config);

        assert_eq!(config.poll_interval_ms, 500);
        assert!(!config.show.network_down);
        assert_eq!(config.color_mode, ColorMode::Theme);
        assert_eq!(config.ui_theme, UiTheme::Dark);
        assert_eq!(config.language, Language::Ko);
        assert!(!config.exclude_virtual_adapters, "count와 exclude는 반대다");
        assert!(config.show.cpu, "자동 실행은 설정 파일과 무관하다");
    }

    /// 창에서 읽은 값과 저장되는 값이 서로 오갈 수 있어야 한다.
    #[test]
    fn draft_round_trips_through_config() {
        let config = Config {
            poll_interval_ms: 2_000,
            show: Show {
                cpu: true,
                memory: false,
                network_down: true,
                network_up: false,
                gpu: false,
                gpu_vram: false,
                disk_usage: false,
                disk_active: false,
                network: None,
            },
            color_mode: ColorMode::Theme,
            ui_theme: UiTheme::Light,
            exclude_virtual_adapters: false,
            language: Language::Ja,
            devices: Devices {
                gpu: "Radeon".into(),
                network: "Ethernet".into(),
                disk: "D:".into(),
            },
        };
        let draft = SettingsDraft::from_config(&config, true);
        assert!(draft.count_virtual, "exclude=false 이면 집계에 포함한다");
        assert!(draft.autostart);
        assert_eq!(draft.language, Language::Ja);

        let mut restored = config.clone();
        for effect in diff(
            &SettingsDraft::from_config(&Config::default(), false),
            &draft,
        ) {
            effect.apply_to_config(&mut restored);
        }
        assert_eq!(restored, config);
    }

    /// 장치를 바꾸면 그 분야의 효과 하나만 나오고, 설정에 그대로 들어간다.
    #[test]
    fn device_changes_are_reported_and_applied() {
        let applied = SettingsDraft::from_config(&Config::default(), false);
        let mut next = applied.clone();
        next.devices.gpu = "Radeon".into();
        next.devices.network = "Wi-Fi".into();
        next.devices.disk = "D:".into();

        let effects = diff(&applied, &next);
        assert_eq!(
            effects,
            vec![
                Effect::GpuDevice("Radeon".into()),
                Effect::NetworkDevice("Wi-Fi".into()),
                Effect::DiskDevice("D:".into()),
            ]
        );

        let mut config = Config::default();
        for effect in &effects {
            effect.apply_to_config(&mut config);
        }
        assert_eq!(config.devices, next.devices);

        // 자동으로 되돌리면 빈 값이 저장된다.
        let back = diff(&next, &applied);
        assert!(back.contains(&Effect::DiskDevice(String::new())));
    }

    #[test]
    fn presets_are_inside_the_allowed_range() {
        for ms in INTERVAL_PRESETS {
            assert!(
                (MIN_POLL_INTERVAL_MS..=MAX_POLL_INTERVAL_MS).contains(&ms),
                "{ms} is outside the allowed range"
            );
            assert!(is_preset(ms));
        }
        assert!(!is_preset(750), "프리셋이 아니면 Custom 이다");
    }
}
