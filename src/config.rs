//! 설정과 상태 파일.
//!
//! - `config.toml`: 사용자가 바꾸는 값. 없으면 기본값으로 만들고, 깨져 있으면 백업 후 기본값.
//! - `state.toml`: 앱이 관리하는 값. 사용자가 편집할 일이 없다.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::icon::Metric;
use crate::logging;

pub const MIN_POLL_INTERVAL_MS: u64 = 250;
pub const MAX_POLL_INTERVAL_MS: u64 = 10_000;
pub const DEFAULT_POLL_INTERVAL_MS: u64 = 1_000;

/// 지표별 색을 쓸지, 한 가지 색으로 갈지.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ColorMode {
    /// 값과 라벨을 테마 전경색 하나로 그린다.
    Theme,
    /// 지표마다 다른 색을 쓴다.
    #[default]
    Metric,
}

/// 어떤 지표를 트레이에 표시할지.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Show {
    pub cpu: bool,
    pub memory: bool,
    pub network_down: bool,
    pub network_up: bool,
    /// v0.1.0의 단일 `network` 항목. 읽어서 아래 두 항목으로 옮기고 다시 쓰지는 않는다.
    #[serde(default, skip_serializing)]
    pub network: Option<bool>,
}

impl Default for Show {
    fn default() -> Self {
        Self {
            cpu: true,
            memory: true,
            network_down: true,
            network_up: true,
            network: None,
        }
    }
}

impl Show {
    pub fn get(&self, metric: Metric) -> bool {
        match metric {
            Metric::Cpu => self.cpu,
            Metric::Memory => self.memory,
            Metric::NetworkDown => self.network_down,
            Metric::NetworkUp => self.network_up,
        }
    }

    pub fn set(&mut self, metric: Metric, value: bool) {
        match metric {
            Metric::Cpu => self.cpu = value,
            Metric::Memory => self.memory = value,
            Metric::NetworkDown => self.network_down = value,
            Metric::NetworkUp => self.network_up = value,
        }
    }

    pub fn count(&self) -> usize {
        [self.cpu, self.memory, self.network_down, self.network_up]
            .into_iter()
            .filter(|enabled| *enabled)
            .count()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    /// 폴링 주기(ms). 250 ~ 10000으로 제한한다.
    pub poll_interval_ms: u64,
    pub show: Show,
    pub color_mode: ColorMode,
    /// 가상 어댑터를 집계에서 뺀다. 물리 NIC와 가상 스위치가 같은 트래픽을
    /// 두 번 세는 것을 막는다.
    pub exclude_virtual_adapters: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            poll_interval_ms: DEFAULT_POLL_INTERVAL_MS,
            show: Show::default(),
            color_mode: ColorMode::default(),
            exclude_virtual_adapters: true,
        }
    }
}

impl Config {
    /// 범위를 벗어난 값을 바로잡고, 바꾼 항목을 사람이 읽을 수 있게 돌려준다.
    pub fn validate(&mut self) -> Vec<String> {
        let mut notes = Vec::new();

        // v0.1.0 설정 파일에는 network 항목이 하나뿐이었다.
        if let Some(legacy) = self.show.network.take() {
            notes.push(format!(
                "network = {legacy} 를 network_down / network_up 으로 옮겼다"
            ));
            self.show.network_down = legacy;
            self.show.network_up = legacy;
        }

        if self.poll_interval_ms < MIN_POLL_INTERVAL_MS {
            notes.push(format!(
                "poll_interval_ms={} -> {} (최소값)",
                self.poll_interval_ms, MIN_POLL_INTERVAL_MS
            ));
            self.poll_interval_ms = MIN_POLL_INTERVAL_MS;
        } else if self.poll_interval_ms > MAX_POLL_INTERVAL_MS {
            notes.push(format!(
                "poll_interval_ms={} -> {} (최대값)",
                self.poll_interval_ms, MAX_POLL_INTERVAL_MS
            ));
            self.poll_interval_ms = MAX_POLL_INTERVAL_MS;
        }

        if self.show.count() == 0 {
            notes.push("표시할 지표가 없어 CPU를 켠다".to_string());
            self.show.cpu = true;
        }

        notes
    }
}

/// 앱이 관리하는 상태.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct State {
    /// 첫 실행 안내를 이미 보여줬는지.
    pub onboarding_shown: bool,
    /// 마지막으로 실행한 버전. 업데이트 후 안내를 다시 띄울지 판단한다.
    pub last_version: String,
}

/// `%APPDATA%\tray-monitor`
pub fn config_dir() -> PathBuf {
    let base = std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("LOCALAPPDATA").map(PathBuf::from))
        .unwrap_or_else(std::env::temp_dir);
    base.join("tray-monitor")
}

/// 설정과 상태를 함께 들고 있는다.
#[derive(Debug)]
pub struct Store {
    pub config: Config,
    pub state: State,
    pub directory: PathBuf,
}

impl Store {
    pub fn load() -> Self {
        Self::load_from(config_dir())
    }

    /// 테스트에서 임시 디렉터리를 쓰기 위해 분리했다.
    pub fn load_from(directory: PathBuf) -> Self {
        let config = load_toml::<Config>(&directory.join("config.toml"));
        let state = load_toml::<State>(&directory.join("state.toml"));
        Self {
            config,
            state,
            directory,
        }
    }

    pub fn save_config(&self) -> Result<(), String> {
        save_toml(&self.directory, "config.toml", &self.config)
    }

    pub fn save_state(&self) -> Result<(), String> {
        save_toml(&self.directory, "state.toml", &self.state)
    }
}

fn load_toml<T: Default + serde::de::DeserializeOwned>(path: &PathBuf) -> T {
    let Ok(text) = std::fs::read_to_string(path) else {
        return T::default();
    };
    match toml::from_str::<T>(&text) {
        Ok(value) => value,
        Err(error) => {
            // 깨진 파일을 덮어써 잃지 않도록 백업해 둔다.
            let backup = path.with_extension("toml.bak");
            let _ = std::fs::rename(path, &backup);
            logging::warn(format!(
                "{} 파싱 실패({error}). 기본값을 쓰고 {} 로 백업했다",
                path.display(),
                backup.display()
            ));
            T::default()
        }
    }
}

fn save_toml<T: Serialize>(directory: &PathBuf, name: &str, value: &T) -> Result<(), String> {
    std::fs::create_dir_all(directory).map_err(|e| e.to_string())?;
    let text = toml::to_string_pretty(value).map_err(|e| e.to_string())?;
    let target = directory.join(name);

    // 반쯤 쓰인 파일이 남으면 다음 시작에 기본값으로 돌아간다. 임시 파일에 쓰고 교체한다.
    let temporary = directory.join(format!("{name}.tmp"));
    std::fs::write(&temporary, text).map_err(|e| e.to_string())?;
    crate::win32::replace_file(&temporary, &target)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("tray-monitor-test-{name}"));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn defaults_are_valid() {
        let mut config = Config::default();
        assert!(config.validate().is_empty());
        assert_eq!(config.poll_interval_ms, DEFAULT_POLL_INTERVAL_MS);
        assert_eq!(config.show.count(), 4);
        assert!(config.exclude_virtual_adapters);
    }

    #[test]
    fn validate_clamps_poll_interval() {
        let mut too_small = Config {
            poll_interval_ms: 10,
            ..Default::default()
        };
        assert_eq!(too_small.validate().len(), 1);
        assert_eq!(too_small.poll_interval_ms, MIN_POLL_INTERVAL_MS);

        let mut too_large = Config {
            poll_interval_ms: 999_999,
            ..Default::default()
        };
        assert_eq!(too_large.validate().len(), 1);
        assert_eq!(too_large.poll_interval_ms, MAX_POLL_INTERVAL_MS);
    }

    #[test]
    fn validate_keeps_at_least_one_metric() {
        let mut config = Config {
            show: Show {
                cpu: false,
                memory: false,
                network_down: false,
                network_up: false,
                network: None,
            },
            ..Default::default()
        };
        let notes = config.validate();
        assert_eq!(notes.len(), 1);
        assert!(
            config.show.cpu,
            "전부 꺼지면 아이콘이 사라지므로 CPU를 켠다"
        );
    }

    #[test]
    fn round_trip_through_toml() {
        let config = Config {
            poll_interval_ms: 500,
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
        let text = toml::to_string_pretty(&config).expect("serialize");
        let parsed: Config = toml::from_str(&text).expect("deserialize");
        assert_eq!(parsed, config);
    }

    /// v0.1.0 설정을 읽으면 단일 network 값이 두 방향으로 복제된다.
    #[test]
    fn legacy_network_setting_is_migrated() {
        let text = "[show]
cpu = true
memory = true
network = false
";
        let mut config: Config = toml::from_str(text).expect("v0.1.0 설정");
        assert!(
            config.show.network_down,
            "마이그레이션 전에는 기본값(true)이다"
        );

        let notes = config.validate();
        assert_eq!(notes.len(), 1, "{notes:?}");
        assert!(!config.show.network_down);
        assert!(!config.show.network_up);

        let written = toml::to_string_pretty(&config).expect("serialize");
        assert!(
            !written.contains("network ="),
            "legacy 항목은 다시 쓰지 않는다:
{written}"
        );
        assert!(written.contains("network_down = false"));
    }

    #[test]
    fn unknown_and_missing_fields_are_tolerated() {
        let text = "poll_interval_ms = 2000\nfuture_option = true\n";
        let config: Config = toml::from_str(text).expect("알 수 없는 키는 무시한다");
        assert_eq!(config.poll_interval_ms, 2000);
        assert_eq!(config.show.count(), 4, "빠진 항목은 기본값");
    }

    #[test]
    fn broken_file_falls_back_and_is_backed_up() {
        let dir = temp_dir("broken");
        std::fs::create_dir_all(&dir).expect("mkdir");
        std::fs::write(dir.join("config.toml"), "poll_interval_ms = = =").expect("write");

        let store = Store::load_from(dir.clone());
        assert_eq!(store.config, Config::default());
        assert!(
            dir.join("config.toml.bak").exists(),
            "깨진 파일은 백업되어야 한다"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn store_round_trips_on_disk() {
        let dir = temp_dir("roundtrip");
        let mut store = Store::load_from(dir.clone());
        store.config.poll_interval_ms = 750;
        store.state.onboarding_shown = true;
        store.save_config().expect("save config");
        store.save_state().expect("save state");

        let reloaded = Store::load_from(dir.clone());
        assert_eq!(reloaded.config.poll_interval_ms, 750);
        assert!(reloaded.state.onboarding_shown);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn show_helpers_match_metric_ids() {
        let mut show = Show::default();
        for metric in Metric::ALL {
            assert!(show.get(metric));
            show.set(metric, false);
            assert!(!show.get(metric));
        }
        assert_eq!(show.count(), 0);
    }

    /// 저장은 임시 파일을 거쳐 교체한다. 임시 파일이 남거나 이전 내용이 살아 있으면 실패다.
    #[test]
    fn saving_replaces_the_file_and_leaves_no_temporary() {
        let dir = temp_dir("atomic");
        let mut store = Store::load_from(dir.clone());
        store.config.poll_interval_ms = 750;
        store.save_config().expect("save");
        store.config.poll_interval_ms = 500;
        store.save_config().expect("save again");

        assert_eq!(Store::load_from(dir.clone()).config.poll_interval_ms, 500);
        assert!(
            !dir.join("config.toml.tmp").exists(),
            "임시 파일이 남으면 안 된다"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}
