//! 설정 창과 트레이 메뉴가 함께 쓰는 편집 상태와 적용 계획.
//!
//! 화면(대화상자)과 저장 구조([Config]) 사이를 순수 함수로 잇는다. 여기 있는 것은 부작용이
//! 없어서 전부 단위 테스트로 검증한다: 값 보정, 마지막 지표 규칙, 바뀐 항목만 뽑아내기.

use crate::config::{ColorMode, Config, MAX_POLL_INTERVAL_MS, MIN_POLL_INTERVAL_MS, Show};
use crate::icon::Metric;

/// 콤보에 넣는 폴링 주기(ms).
pub const INTERVAL_PRESETS: [u64; 6] = [250, 500, 1_000, 2_000, 5_000, 10_000];

/// 편집 중인 값. 저장하기 전 상태이므로 [Config]와 분리해 둔다.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SettingsDraft {
    pub poll_interval_ms: u64,
    pub show: Show,
    pub color_mode: ColorMode,
    /// 가상 어댑터를 집계에 넣는가. `Config`에는 반대 의미로 저장된다.
    pub count_virtual: bool,
    pub autostart: bool,
}

impl SettingsDraft {
    pub fn from_config(config: &Config, autostart: bool) -> Self {
        Self {
            poll_interval_ms: config.poll_interval_ms,
            show: config.show,
            color_mode: config.color_mode,
            count_virtual: !config.exclude_virtual_adapters,
            autostart,
        }
    }

    /// 범위를 벗어난 값을 바로잡고, 바꾼 항목을 사람이 읽을 수 있게 돌려준다.
    pub fn validate(&mut self) -> Vec<String> {
        let mut notes = Vec::new();

        let clamped = self
            .poll_interval_ms
            .clamp(MIN_POLL_INTERVAL_MS, MAX_POLL_INTERVAL_MS);
        if clamped != self.poll_interval_ms {
            notes.push(format!(
                "poll_interval_ms {} -> {clamped}",
                self.poll_interval_ms
            ));
            self.poll_interval_ms = clamped;
        }

        if self.show.count() == 0 {
            notes.push("no readout is shown, turning CPU on".to_string());
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
    CountVirtual(bool),
    Autostart(bool),
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
            Effect::CountVirtual(count) => config.exclude_virtual_adapters = !*count,
            Effect::Autostart(_) => {}
        }
    }
}

/// 지표 표시를 바꾼다. 마지막 남은 하나는 끌 수 없다.
///
/// 전부 끄면 트레이에서 아이콘이 사라져 되돌릴 방법이 없다. 설정 창과 트레이 메뉴가 같은
/// 규칙을 쓰도록 여기에 둔다.
pub fn set_visible(show: &mut Show, metric: Metric, visible: bool) -> Result<(), String> {
    if !visible && show.get(metric) && show.count() <= 1 {
        return Err("the last readout cannot be turned off".to_string());
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

    if applied.count_virtual != next.count_virtual {
        effects.push(Effect::CountVirtual(next.count_virtual));
    }

    if applied.autostart != next.autostart {
        effects.push(Effect::Autostart(next.autostart));
    }

    effects
}

/// 콤보에 보여줄 주기 표기.
pub fn interval_label(ms: u64) -> String {
    match ms {
        1_000 => "1 second".to_string(),
        _ if ms % 1_000 == 0 => format!("{} seconds", ms / 1_000),
        _ => format!("{ms} ms"),
    }
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
        assert_eq!(too_fast.validate().len(), 1);
        assert_eq!(too_fast.poll_interval_ms, MIN_POLL_INTERVAL_MS);

        let mut too_slow = draft(60_000);
        assert_eq!(too_slow.validate().len(), 1);
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
        let notes = empty.validate();
        assert_eq!(notes.len(), 1);
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
        assert!(error.contains("last readout"), "{error}");
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

        let mut virtual_adapters = applied.clone();
        virtual_adapters.count_virtual = true;
        assert_eq!(
            diff(&applied, &virtual_adapters),
            vec![Effect::CountVirtual(true)]
        );

        let mut autostart = applied.clone();
        autostart.autostart = true;
        assert_eq!(diff(&applied, &autostart), vec![Effect::Autostart(true)]);
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
        Effect::CountVirtual(true).apply_to_config(&mut config);
        Effect::Autostart(true).apply_to_config(&mut config);

        assert_eq!(config.poll_interval_ms, 500);
        assert!(!config.show.network_down);
        assert_eq!(config.color_mode, ColorMode::Theme);
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
                network: None,
            },
            color_mode: ColorMode::Theme,
            exclude_virtual_adapters: false,
        };
        let draft = SettingsDraft::from_config(&config, true);
        assert!(draft.count_virtual, "exclude=false 이면 집계에 포함한다");
        assert!(draft.autostart);

        let mut restored = config.clone();
        for effect in diff(
            &SettingsDraft::from_config(&Config::default(), false),
            &draft,
        ) {
            effect.apply_to_config(&mut restored);
        }
        assert_eq!(restored, config);
    }

    #[test]
    fn interval_labels_read_naturally() {
        assert_eq!(interval_label(250), "250 ms");
        assert_eq!(interval_label(1_000), "1 second");
        assert_eq!(interval_label(5_000), "5 seconds");
        assert_eq!(interval_label(750), "750 ms");
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
