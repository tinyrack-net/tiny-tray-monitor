//! 앱 상태와 갱신 오케스트레이션.

use std::time::{Duration, Instant};

use tray_icon::TrayIconEvent;
use tray_icon::menu::MenuEvent;

use crate::config::Store;
use crate::icon::Metric;
use crate::logging;
use crate::metrics::{self, Sampler, format};
use crate::startup;
use crate::theme;
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
    ticks: u64,
    last_cpu: Option<String>,
    last_memory: Option<String>,
    last_network: Option<String>,
}

impl App {
    pub fn new(px: i32) -> Result<Self, String> {
        let mut store = Store::load();
        for note in store.config.validate() {
            logging::warn(format!("설정 보정: {note}"));
        }

        let palette = theme::current();
        let autostart = startup::is_enabled();
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
            ticks: 0,
            last_cpu: None,
            last_memory: None,
            last_network: None,
        })
    }

    pub fn poll_interval(&self) -> Duration {
        Duration::from_millis(self.store.config.poll_interval_ms)
    }

    pub fn should_exit(&self) -> bool {
        self.exiting
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
            self.last_network = Some(format::rate(net.down));
            self.tray.update(
                Metric::Network,
                format::rate(net.down),
                format!(
                    "Network down {}  up {}",
                    format::rate_long(net.down),
                    format::rate_long(net.up)
                ),
            );
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
            "{}번째 갱신: cpu {} mem {} net {} | 글리프 캐시 {:.0}% (hit {} miss {} entries {}) 아이콘 교체 {}회",
            self.ticks,
            self.last_cpu.as_deref().unwrap_or("-"),
            self.last_memory.as_deref().unwrap_or("-"),
            self.last_network.as_deref().unwrap_or("-"),
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
                self.toggle_metric(*metric, index);
                return;
            }
        }

        match id {
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

    fn toggle_metric(&mut self, metric: Metric, index: usize) {
        let next = !self.tray.is_visible(metric);
        if !next && self.store.config.show.count() <= 1 {
            // 모두 끄면 트레이에서 사라져 되돌릴 방법이 없다.
            logging::warn("마지막 남은 지표는 끌 수 없다");
            self.tray.menu_handles().show[index].set_checked(true);
            return;
        }

        self.tray.set_visible(metric, next);
        self.store.config.show.set(metric, next);
        self.tray.menu_handles().show[index].set_checked(next);
        logging::info(format!("{} 표시 = {next}", metric.id()));

        if let Err(error) = self.store.save_config() {
            logging::warn(format!("설정을 저장하지 못했다: {error}"));
        }
    }

    fn toggle_autostart(&mut self) {
        let enabled = startup::is_enabled();
        match startup::set(!enabled) {
            Ok(()) => logging::info(format!("자동 실행 = {}", !enabled)),
            Err(error) => {
                logging::error(format!("자동 실행을 바꾸지 못했다: {error}"));
                crate::win32::inform(
                    "tray-monitor",
                    &format!("Could not update sign-in startup.\n\n{error}"),
                );
            }
        }
        // 실제 레지스트리 상태를 다시 읽어 체크 표시를 맞춘다.
        self.tray
            .menu_handles()
            .autostart
            .set_checked(startup::is_enabled());
    }

    fn toggle_virtual_adapters(&mut self) {
        let excluded = self.store.config.exclude_virtual_adapters;
        self.store.config.exclude_virtual_adapters = !excluded;
        self.sampler.set_exclude_virtual_adapters(!excluded);
        self.tray
            .menu_handles()
            .virtual_adapters
            .set_checked(excluded);
        logging::info(format!(
            "가상 어댑터 집계 = {}",
            if excluded { "포함" } else { "제외" }
        ));
        if let Err(error) = self.store.save_config() {
            logging::warn(format!("설정을 저장하지 못했다: {error}"));
        }
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
