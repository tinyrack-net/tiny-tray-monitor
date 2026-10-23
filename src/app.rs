//! 앱 상태와 갱신 오케스트레이션.

use std::time::{Duration, Instant};

use tray_icon::TrayIconEvent;

use crate::config::Store;
use crate::i18n;
use crate::icon::Metric;
use crate::logging;
use crate::metrics::{self, Sampler, format};
use crate::settings::model::{self, Effect, SettingsDraft};
use crate::startup;
use crate::theme;
use crate::timer;
use crate::tray::{self, Tray};

/// 테마는 자주 바뀌지 않으므로 이 주기로만 다시 읽는다.
const THEME_POLL: Duration = Duration::from_secs(10);

pub struct App {
    store: Store,
    sampler: Sampler,
    tray: Tray,
    palette: theme::Palette,
    last_tick: Option<Instant>,
    last_theme_check: Instant,
    exiting: bool,
    settings_requested: bool,
    ticks: u64,
    last_cpu: Option<String>,
    last_memory: Option<String>,
    last_network_down: Option<String>,
    last_network_up: Option<String>,
    last_gpu: Option<String>,
    last_gpu_vram: Option<String>,
    last_disk_usage: Option<String>,
    last_disk_active: Option<String>,
}

impl App {
    pub fn new(px: i32) -> Result<Self, String> {
        let mut store = Store::load();
        for note in store.config.validate() {
            logging::warn(format!("config corrected: {note}"));
        }
        // 창·트레이 메뉴·툴팁이 쓸 언어를 먼저 정한다.
        i18n::set_language(i18n::resolve(store.config.language));

        let palette = theme::current();
        let autostart = startup::is_enabled();
        // 트레이 메뉴는 창과 같은 테마를 쓴다(메뉴는 창이 아니라 앱 단위로 칠해진다).
        theme::set_ui_theme(store.config.ui_theme);
        crate::win32::dark::set_app_mode(theme::ui_dark());
        logging::info(format!(
            "config poll={}ms show={:?} color={:?} autostart={autostart} language={:?}",
            store.config.poll_interval_ms,
            store.config.show,
            store.config.color_mode,
            store.config.language
        ));

        let tray = Tray::new(px, &store.config, palette)?;
        let sampler = Sampler::new(&store.config.devices);

        // 기본값 파일을 남겨 사용자가 편집할 수 있게 한다.
        if let Err(error) = store.save_config() {
            logging::warn(format!("could not save the config: {error}"));
        }

        Ok(Self {
            store,
            sampler,
            tray,
            palette,
            last_tick: None,
            last_theme_check: Instant::now(),
            exiting: false,
            settings_requested: false,
            ticks: 0,
            last_cpu: None,
            last_memory: None,
            last_network_down: None,
            last_network_up: None,
            last_gpu: None,
            last_gpu_vram: None,
            last_disk_usage: None,
            last_disk_active: None,
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
                    "re-priming the counters after a {:.1}s gap",
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
                    "taskbar switched to light"
                } else {
                    "taskbar switched to dark"
                });
                self.palette = palette;
                self.tray.set_style(palette, self.store.config.color_mode);
                crate::win32::dark::set_app_mode(theme::ui_dark());
            }
        }

        // 배율은 실행 중에도 바뀐다. 시작할 때 잰 크기를 계속 쓰면 셸이 옛 크기의 아이콘을
        // 늘려 흐릿해진다(docs/SPIKE.md §16). 확인은 창 하나를 찾는 정도라 매 틱 한다.
        // 타이머 창은 message-only라 WM_DPICHANGED/WM_SETTINGCHANGE를 받지 못한다.
        let px = crate::win32::tray_icon_size();
        if let Some(previous) = self.tray.set_px(px) {
            logging::info(format!("tray icon size {previous}px -> {px}px"));
        }

        let sample = self.sampler.sample();
        self.apply(sample);
        self.report_stats();
    }

    fn apply(&mut self, sample: metrics::Sample) {
        if let Some(cpu) = sample.cpu_percent {
            let value = format::percent_reading(cpu);
            self.last_cpu = Some(value.compact());
            let tooltip = i18n::cpu_tooltip(&format::percent(cpu));
            self.tray.update(Metric::Cpu, value, tooltip);
        }

        if let Some(memory) = sample.memory {
            let value = format::percent_reading(memory.percent);
            self.last_memory = Some(value.compact());
            let tooltip = i18n::memory_tooltip(
                &format::percent(memory.percent),
                &format::bytes(memory.used),
                &format::bytes(memory.total),
            );
            self.tray.update(Metric::Memory, value, tooltip);
        }

        if let Some(net) = sample.net {
            // 방향별로 아이콘을 나눠 쓴다. 툴팁은 두 방향을 함께 보여준다.
            let down = format::rate_reading(net.down);
            let up = format::rate_reading(net.up);
            let (down_long, up_long) = (format::rate_long(net.down), format::rate_long(net.up));
            let tooltip = match self.sampler.network_device() {
                Some(device) => i18n::network_device_tooltip(device, &down_long, &up_long),
                None => i18n::network_tooltip(&down_long, &up_long),
            };
            self.last_network_down = Some(down.compact());
            self.last_network_up = Some(up.compact());
            self.tray.update(Metric::NetworkDown, down, tooltip.clone());
            self.tray.update(Metric::NetworkUp, up, tooltip);
        }

        if let Some(gpu) = sample.gpu_percent {
            let value = format::percent_reading(gpu);
            self.last_gpu = Some(value.compact());
            let adapter = self
                .sampler
                .gpu_adapter()
                .map(|adapter| adapter.name.as_str())
                .unwrap_or_else(|| i18n::metric_label(Metric::Gpu));
            let tooltip = i18n::gpu_tooltip(adapter, &format::percent(gpu));
            self.tray.update(Metric::Gpu, value, tooltip);
        }

        if let Some(vram) = sample.gpu_vram {
            let value = format::percent_reading(vram.percent);
            self.last_gpu_vram = Some(value.compact());
            let tooltip = i18n::vram_tooltip(
                &format::percent(vram.percent),
                &format::bytes(vram.used),
                &format::bytes(vram.total),
            );
            self.tray.update(Metric::GpuVram, value, tooltip);
        }

        if let Some(usage) = sample.disk_usage {
            let value = format::percent_reading(usage.percent);
            self.last_disk_usage = Some(value.compact());
            let tooltip = i18n::disk_usage_tooltip(
                &metrics::disk::letter_value(usage.letter),
                &format::percent(usage.percent),
                &format::bytes(usage.used),
                &format::bytes(usage.total),
            );
            self.tray.update(Metric::DiskUsage, value, tooltip);
        }

        if let Some(activity) = sample.disk_active {
            let value = format::percent_reading(activity.percent);
            self.last_disk_active = Some(value.compact());
            let tooltip = i18n::disk_active_tooltip(
                &metrics::disk::letter_value(activity.letter),
                &format::percent(activity.percent),
                &format::rate_long(activity.read),
                &format::rate_long(activity.write),
            );
            self.tray.update(Metric::DiskActive, value, tooltip);
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
            "update #{}: cpu {} mem {} net down {} up {} gpu {} vram {} disk {} io {} | glyph cache {:.0}% (hit {} miss {} entries {}) icon swaps {}",
            self.ticks,
            self.last_cpu.as_deref().unwrap_or("-"),
            self.last_memory.as_deref().unwrap_or("-"),
            self.last_network_down.as_deref().unwrap_or("-"),
            self.last_network_up.as_deref().unwrap_or("-"),
            self.last_gpu.as_deref().unwrap_or("-"),
            self.last_gpu_vram.as_deref().unwrap_or("-"),
            self.last_disk_usage.as_deref().unwrap_or("-"),
            self.last_disk_active.as_deref().unwrap_or("-"),
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
            tray::MENU_EXIT => {
                logging::info("exit chosen from the menu");
                self.exiting = true;
            }
            other => logging::warn(format!("unknown menu event {other}")),
        }
    }

    /// 지금 적용되어 있는 값. 설정 창과 트레이 메뉴가 같은 기준을 쓴다.
    pub fn draft(&self) -> SettingsDraft {
        SettingsDraft::from_config(&self.store.config, startup::is_enabled())
    }

    /// 설정 창을 보여 달라는 요청. 트레이 메뉴와 두 번째 실행이 같은 정책을 따른다.
    ///
    /// - 열려 있으면 그 창을 앞으로 가져온다(새 창을 예약하지 않는다).
    /// - 닫혀 있으면 열기를 예약만 한다. 여기서 바로 열면 안 된다: 메뉴 이벤트는 `App`을
    ///   빌린 채 처리되고, 대여 중에 모달 루프에 들어가면 그동안 타이머 틱이 전부 버려진다.
    pub fn request_settings(&mut self) {
        if crate::settings::dialog::focus() {
            return;
        }
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
            logging::warn(format!("could not save the config: {error}"));
            problems.push(i18n::save_failed(&error));
        }
        problems
    }

    /// 트레이 메뉴에서 바뀐 값을 반영한다.
    ///
    /// 메뉴에는 오류를 보여 줄 창이 없으므로 알림 창을 쓴다. 설정 창은 자기 상태줄에 쓴다.
    fn apply_from_menu(&mut self, effects: &[Effect]) {
        let problems = self.apply_effects(effects);
        // 설정 창이 열려 있으면 체크박스와 미리보기도 맞춘다.
        crate::settings::dialog::refresh();
        if !problems.is_empty() {
            crate::win32::inform(crate::PRODUCT_NAME, &problems.join("\n\n"));
        }
    }

    /// 체크 표시를 실제 값에서 다시 맞춘다. 창과 메뉴가 같은 함수를 쓴다.
    pub fn sync_menu_checks(&self) {
        self.tray.menu_handles().sync(&self.store.config);
    }

    fn apply_effect(&mut self, effect: &Effect) -> Option<String> {
        effect.apply_to_config(&mut self.store.config);

        match effect {
            Effect::Interval(ms) => {
                // 대화상자 안에서도 즉시 반영되어야 한다.
                timer::set_interval(*ms as u32);
                logging::info(format!("poll interval = {ms}ms"));
                None
            }
            Effect::Visible(metric, visible) => {
                self.tray.set_visible(*metric, *visible);
                logging::info(format!("show {} = {visible}", metric.id()));
                None
            }
            Effect::ColorMode(mode) => {
                self.tray.set_style(self.palette, *mode);
                logging::info(format!("color mode = {mode:?}"));
                None
            }
            Effect::UiTheme(ui_theme) => {
                theme::set_ui_theme(*ui_theme);
                crate::win32::dark::set_app_mode(theme::ui_dark());
                logging::info(format!("window theme = {ui_theme:?}"));
                None
            }
            Effect::Language(language) => {
                i18n::set_language(i18n::resolve(*language));
                self.tray.relabel();
                logging::info(format!("language = {language:?}"));
                None
            }
            Effect::GpuDevice(name) => {
                self.sampler.set_gpu_device(name);
                logging::info(format!("gpu device = {}", device_name(name)));
                None
            }
            Effect::NetworkDevice(name) => {
                self.sampler.set_network_device(name);
                logging::info(format!("network device = {}", device_name(name)));
                None
            }
            Effect::DiskDevice(drive) => {
                self.sampler.set_disk_device(drive);
                logging::info(format!("disk device = {}", device_name(drive)));
                None
            }
            Effect::Autostart(enabled) => match startup::set(*enabled) {
                Ok(()) => {
                    logging::info(format!("run at sign-in = {enabled}"));
                    None
                }
                Err(error) => {
                    logging::error(format!("could not change run at sign-in: {error}"));
                    Some(i18n::autostart_failed(&error))
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
            Err(model::LastReadout) => {
                // 클릭이 무시됐으므로 체크 표시를 실제 값으로 되돌린다.
                logging::warn(format!(
                    "the last readout cannot be turned off: {}",
                    metric.id()
                ));
                self.sync_menu_checks();
            }
        }
    }

    /// 트레이 아이콘 이벤트를 비운다. 메뉴 이벤트는 [crate::runtime::menu_event]가 받는다.
    pub fn drain_tray_events(&mut self) {
        while let Ok(event) = TrayIconEvent::receiver().try_recv() {
            logging::info(format!("tray event {event:?}"));
        }
    }
}

/// 로그에 남길 장치 이름. 빈 값은 자동이다.
fn device_name(value: &str) -> &str {
    if value.is_empty() { "auto" } else { value }
}
