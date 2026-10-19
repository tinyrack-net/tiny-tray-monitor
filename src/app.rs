//! 앱 상태와 갱신 오케스트레이션.

use std::time::{Duration, Instant};

use tray_icon::TrayIconEvent;
use tray_icon::menu::MenuEvent;

use crate::config::Store;
use crate::icon::Metric;
use crate::logging;
use crate::metrics::{self, Sampler, format};
use crate::settings::model::{self, Effect, SettingsDraft};
use crate::startup;
use crate::theme;
use crate::timer;
use crate::tray::{self, Tray, onboard};

/// 테마는 자주 바뀌지 않으므로 이 주기로만 다시 읽는다.
const THEME_POLL: Duration = Duration::from_secs(10);

pub struct App {
    store: Store,
    sampler: Sampler,
    tray: Tray,
    palette: theme::Palette,
    last_tick: Option<Instant>,
    last_theme_check: Instant,
    onboard_pending: bool,
    exiting: bool,
    settings_requested: bool,
    ticks: u64,
    last_cpu: Option<String>,
    last_memory: Option<String>,
    last_network_down: Option<String>,
    last_network_up: Option<String>,
}

impl App {
    pub fn new(px: i32) -> Result<Self, String> {
        let mut store = Store::load();
        for note in store.config.validate() {
            logging::warn(format!("설정 보정: {note}"));
        }

        let palette = theme::current();
        let autostart = startup::is_enabled();
        // 트레이 메뉴도 작업 표시줄 테마를 따른다(메뉴는 창이 아니라 앱 단위로 칠해진다).
        crate::win32::dark::set_app_mode(!palette.light);
        logging::info(format!(
            "설정 poll={}ms show={:?} color={:?} exclude_virtual={} autostart={autostart}",
            store.config.poll_interval_ms,
            store.config.show,
            store.config.color_mode,
            store.config.exclude_virtual_adapters
        ));

        let tray = Tray::new(px, &store.config, palette, autostart)?;
        let sampler = Sampler::new(store.config.exclude_virtual_adapters);
        let promotion = onboard::promotion_state();
        let onboard_pending = onboard::should_onboard(&store.state, promotion);
        logging::info(format!(
            "승격 상태 {promotion:?}, 첫 실행 안내 {onboard_pending}"
        ));

        // 기본값 파일을 남겨 사용자가 편집할 수 있게 한다.
        if let Err(error) = store.save_config() {
            logging::warn(format!("설정을 저장하지 못했다: {error}"));
        }

        Ok(Self {
            store,
            sampler,
            tray,
            palette,
            last_tick: None,
            last_theme_check: Instant::now(),
            onboard_pending,
            exiting: false,
            settings_requested: false,
            ticks: 0,
            last_cpu: None,
            last_memory: None,
            last_network_down: None,
            last_network_up: None,
        })
    }

    pub fn poll_interval(&self) -> Duration {
        Duration::from_millis(self.store.config.poll_interval_ms)
    }

    pub fn should_exit(&self) -> bool {
        self.exiting
    }

    /// 종료 요청. installer가 업그레이드 전에 보내거나 메뉴의 Quit이 쓴다.
    pub fn request_exit(&mut self) {
        self.exiting = true;
    }

    /// 샘플링 주기마다 호출된다.
    pub fn tick(&mut self) {
        let now = Instant::now();

        // 절전 복귀나 긴 정지 뒤에는 누적 카운터 델타가 폭주하므로 기준점을 다시 잡는다.
        if let Some(previous) = self.last_tick {
            let gap = now.duration_since(previous);
            if gap > self.poll_interval() * 4 {
                logging::info(format!(
                    "긴 공백 {:.1}초 뒤라 기준점을 다시 잡는다",
                    gap.as_secs_f64()
                ));
                self.sampler.prime();
            }
        }
        self.last_tick = Some(now);

        if now.duration_since(self.last_theme_check) >= THEME_POLL {
            self.last_theme_check = now;
            let palette = theme::current();
            if palette != self.palette {
                logging::info(if palette.light {
                    "밝은 작업 표시줄로 바뀌었다"
                } else {
                    "어두운 작업 표시줄로 바뀌었다"
                });
                self.palette = palette;
                self.tray.set_style(palette, self.store.config.color_mode);
                crate::win32::dark::set_app_mode(!palette.light);
            }
        }

        let sample = self.sampler.sample();
        self.apply(sample);
        self.report_stats();

        if self.onboard_pending {
            self.onboard_pending = false;
            self.run_onboarding();
        }
    }

    fn apply(&mut self, sample: metrics::Sample) {
        if let Some(cpu) = sample.cpu_percent {
            self.last_cpu = Some(format::percent(cpu));
            self.tray.update(
                Metric::Cpu,
                format::percent(cpu),
                format!("CPU {} busy", format::percent(cpu)),
            );
        }

        if let Some(memory) = sample.memory {
            self.last_memory = Some(format::percent(memory.percent));
            self.tray.update(
                Metric::Memory,
                format::percent(memory.percent),
                format!(
                    "Memory {} used  ({} of {})",
                    format::percent(memory.percent),
                    format::bytes(memory.used),
                    format::bytes(memory.total)
                ),
            );
        }

        if let Some(net) = sample.net {
            // 방향별로 아이콘을 나눠 쓴다. 툴팁은 두 방향을 함께 보여준다.
            let down = format::rate(net.down);
            let up = format::rate(net.up);
            let tooltip = format!(
                "Network down {}  up {}",
                format::rate_long(net.down),
                format::rate_long(net.up)
            );
            self.last_network_down = Some(down.clone());
            self.last_network_up = Some(up.clone());
            self.tray.update(Metric::NetworkDown, down, tooltip.clone());
            self.tray.update(Metric::NetworkUp, up, tooltip);
        }
    }

    /// 주기적으로 상태를 남긴다. 값이 멈추면(타이머가 죽으면) 로그도 멈추므로 진단에 쓴다.
    fn report_stats(&mut self) {
        self.ticks += 1;
        if self.ticks != 1 && self.ticks % 60 != 0 {
            return;
        }
        let stats = self.tray.cache_stats();
        logging::info(format!(
            "{}번째 갱신: cpu {} mem {} net ↓{} ↑{} | 글리프 캐시 {:.0}% (hit {} miss {} entries {}) 아이콘 교체 {}회",
            self.ticks,
            self.last_cpu.as_deref().unwrap_or("-"),
            self.last_memory.as_deref().unwrap_or("-"),
            self.last_network_down.as_deref().unwrap_or("-"),
            self.last_network_up.as_deref().unwrap_or("-"),
            stats.hit_ratio(),
            stats.hits,
            stats.misses,
            stats.entries,
            self.tray.render_count()
        ));
    }

    /// 메뉴 항목 클릭 처리.
    pub fn handle_menu(&mut self, id: &str) {
        for (index, metric) in Metric::ALL.iter().enumerate() {
            if id == tray::MENU_SHOW[index] {
                self.toggle_metric(*metric);
                return;
            }
        }

        match id {
            tray::MENU_SETTINGS => self.request_settings(),
            tray::MENU_AUTOSTART => self.toggle_autostart(),
            tray::MENU_VIRTUAL => self.toggle_virtual_adapters(),
            tray::MENU_OPEN_CONFIG => tray::open_config_file(),
            tray::MENU_EXIT => {
                logging::info("메뉴에서 종료를 선택했다");
                self.exiting = true;
            }
            other => logging::warn(format!("알 수 없는 메뉴 이벤트 {other}")),
        }
    }

    /// 지금 적용되어 있는 값. 설정 창과 트레이 메뉴가 같은 기준을 쓴다.
    pub fn draft(&self) -> SettingsDraft {
        SettingsDraft::from_config(&self.store.config, startup::is_enabled())
    }

    /// 설정 창을 열어 달라고 요청만 한다.
    ///
    /// 여기서 바로 열면 안 된다: 메뉴 이벤트는 `App`을 빌린 채 처리되고, 대여 중에 모달
    /// 루프에 들어가면 그동안 타이머 틱이 전부 버려져 트레이가 멈춘다.
    pub fn request_settings(&mut self) {
        self.settings_requested = true;
    }

    /// 요청이 있으면 가져가고 지운다.
    pub fn take_settings_request(&mut self) -> bool {
        std::mem::take(&mut self.settings_requested)
    }

    /// 바뀐 항목을 반영한다. 설정 창과 트레이 메뉴가 **이 한 경로만** 쓴다.
    ///
    /// 반영하지 못한 항목이 있으면 사람이 읽을 이유를 돌려준다. **여기서 알림 창을 띄우지
    /// 않는다.** 알림 창은 중첩 루프를 돌리고, 그 동안 `App` 대여가 풀리지 않아 타이머 틱이
    /// 전부 버려진다(실측). 무엇으로 알릴지는 부른 쪽이 정한다.
    pub fn apply_effects(&mut self, effects: &[Effect]) -> Vec<String> {
        if effects.is_empty() {
            return Vec::new();
        }

        let mut problems = Vec::new();
        for effect in effects {
            if let Some(problem) = self.apply_effect(effect) {
                problems.push(problem);
            }
        }

        self.sync_menu_checks();
        if let Err(error) = self.store.save_config() {
            logging::warn(format!("설정을 저장하지 못했다: {error}"));
            problems.push(format!("Could not save the settings file: {error}"));
        }
        problems
    }

    /// 트레이 메뉴에서 바뀐 값을 반영한다.
    ///
    /// 메뉴에는 오류를 보여 줄 창이 없으므로 알림 창을 쓴다. 설정 창은 자기 상태줄에 쓴다.
    fn apply_from_menu(&mut self, effects: &[Effect]) {
        let problems = self.apply_effects(effects);
        if !problems.is_empty() {
            crate::win32::inform("tray-monitor", &problems.join("\n\n"));
        }
    }

    /// 체크 표시를 실제 값에서 다시 맞춘다. 창과 메뉴가 같은 함수를 쓴다.
    pub fn sync_menu_checks(&self) {
        self.tray
            .menu_handles()
            .sync(&self.store.config, startup::is_enabled());
    }

    fn apply_effect(&mut self, effect: &Effect) -> Option<String> {
        effect.apply_to_config(&mut self.store.config);

        match effect {
            Effect::Interval(ms) => {
                // 대화상자 안에서도 즉시 반영되어야 한다.
                timer::set_interval(*ms as u32);
                logging::info(format!("폴링 주기 = {ms}ms"));
                None
            }
            Effect::Visible(metric, visible) => {
                self.tray.set_visible(*metric, *visible);
                logging::info(format!("{} 표시 = {visible}", metric.id()));
                None
            }
            Effect::ColorMode(mode) => {
                self.tray.set_style(self.palette, *mode);
                logging::info(format!("색 모드 = {mode:?}"));
                None
            }
            Effect::CountVirtual(count) => {
                self.sampler.set_exclude_virtual_adapters(!count);
                logging::info(format!(
                    "가상 어댑터 집계 = {}",
                    if *count { "포함" } else { "제외" }
                ));
                None
            }
            Effect::Autostart(enabled) => match startup::set(*enabled) {
                Ok(()) => {
                    logging::info(format!("자동 실행 = {enabled}"));
                    None
                }
                Err(error) => {
                    logging::error(format!("자동 실행을 바꾸지 못했다: {error}"));
                    Some(format!("Could not update sign-in startup: {error}"))
                }
            },
        }
    }

    fn toggle_metric(&mut self, metric: Metric) {
        let applied = self.draft();
        let mut next = applied.clone();
        let wanted = !applied.show.get(metric);

        match model::set_visible(&mut next.show, metric, wanted) {
            Ok(()) => self.apply_from_menu(&model::diff(&applied, &next)),
            Err(reason) => {
                // 클릭이 무시됐으므로 체크 표시를 실제 값으로 되돌린다.
                logging::warn(format!("{reason}: {}", metric.id()));
                self.sync_menu_checks();
            }
        }
    }

    fn toggle_autostart(&mut self) {
        let applied = self.draft();
        let mut next = applied.clone();
        next.autostart = !applied.autostart;
        self.apply_from_menu(&model::diff(&applied, &next));
    }

    fn toggle_virtual_adapters(&mut self) {
        let applied = self.draft();
        let mut next = applied.clone();
        next.count_virtual = !applied.count_virtual;
        self.apply_from_menu(&model::diff(&applied, &next));
    }

    fn run_onboarding(&mut self) {
        let promotion = onboard::promotion_state();
        if promotion != onboard::Promotion::All {
            onboard::show_onboarding(promotion);
        }
        self.store.state.onboarding_shown = true;
        self.store.state.last_version = env!("CARGO_PKG_VERSION").to_string();
        if let Err(error) = self.store.save_state() {
            logging::warn(format!("상태를 저장하지 못했다: {error}"));
        }
    }

    /// 트레이/메뉴 이벤트를 비운다. 클릭 메시지가 도착하면 즉시 처리된다.
    pub fn drain_events(&mut self) {
        while let Ok(event) = MenuEvent::receiver().try_recv() {
            self.handle_menu(&event.id.0);
        }
        while let Ok(event) = TrayIconEvent::receiver().try_recv() {
            logging::info(format!("트레이 이벤트 {event:?}"));
        }
    }
}
