//! 파일 로그.
//!
//! 릴리스 빌드는 콘솔이 없으므로(`windows_subsystem`) 문제를 추적할 수 없다.
//! `%LOCALAPPDATA%\tray-monitor\log.txt`에 남기고, 일정 크기를 넘으면 한 번 갈아치운다.

use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

/// 이 크기를 넘으면 `log.1.txt`로 옮기고 새로 쓴다.
const MAX_BYTES: u64 = 256 * 1024;

static LOGGER: OnceLock<Mutex<Option<File>>> = OnceLock::new();

/// `%LOCALAPPDATA%\tray-monitor` (패키지에서는 `LocalState`)
pub fn data_dir() -> PathBuf {
    crate::platform::data_dir()
}

pub fn log_path() -> PathBuf {
    data_dir().join("log.txt")
}

/// 로그를 열고 시작을 알린다. 실패해도 앱은 계속 돈다.
pub fn init() {
    let path = log_path();
    if let Some(dir) = path.parent() {
        let _ = fs::create_dir_all(dir);
    }
    rotate_if_needed(&path);

    let file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .ok();
    let _ = LOGGER.set(Mutex::new(file));

    info(format!(
        "--- {} v{} 시작 ({}) ---",
        crate::PRODUCT_NAME,
        env!("CARGO_PKG_VERSION"),
        std::env::current_exe()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|_| "<unknown>".into())
    ));
}

fn rotate_if_needed(path: &PathBuf) {
    let too_big = fs::metadata(path)
        .map(|m| m.len() > MAX_BYTES)
        .unwrap_or(false);
    if too_big {
        let _ = fs::rename(path, path.with_file_name("log.1.txt"));
    }
}

fn timestamp() -> String {
    // 추가 의존성 없이 파일 수정 시각만으로는 부족하므로 SystemTime 기반으로 만든다.
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let (days, rest) = (secs / 86_400, secs % 86_400);
    let (h, m, s) = (rest / 3600, (rest % 3600) / 60, rest % 60);
    // 1970-01-01 기준 일수. 표시용이므로 정확한 달력 변환은 하지 않는다.
    format!("epoch+{days}d {h:02}:{m:02}:{s:02}")
}

fn write(level: &str, message: &str) {
    let line = format!("[{}] {level:<5} {message}\n", timestamp());
    if let Some(logger) = LOGGER.get() {
        if let Ok(mut guard) = logger.lock() {
            if let Some(file) = guard.as_mut() {
                let _ = file.write_all(line.as_bytes());
                let _ = file.flush();
            }
        }
    }
    if cfg!(debug_assertions) {
        print!("{line}");
    }
}

pub fn info(message: impl AsRef<str>) {
    write("INFO", message.as_ref());
}

pub fn warn(message: impl AsRef<str>) {
    write("WARN", message.as_ref());
}

pub fn error(message: impl AsRef<str>) {
    write("ERROR", message.as_ref());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn data_dir_is_under_local_appdata() {
        let dir = data_dir();
        assert!(dir.ends_with("tray-monitor"), "{}", dir.display());
        if let Some(base) = std::env::var_os("LOCALAPPDATA") {
            assert!(dir.starts_with(PathBuf::from(base)));
        }
    }

    #[test]
    fn logging_does_not_panic_before_init() {
        info("초기화 전 기록은 조용히 버려진다");
        warn("경고");
        error("오류");
    }

    #[test]
    fn log_line_has_level_and_time() {
        let line = format!("[{}] {:<5} {}", timestamp(), "INFO", "hello");
        assert!(line.contains("INFO"));
        assert!(line.contains("hello"));
    }
}
