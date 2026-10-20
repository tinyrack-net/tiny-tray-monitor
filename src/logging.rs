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

fn log_path() -> PathBuf {
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
        "--- {} v{} started ({}) ---",
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
        info("records before init are dropped silently");
        warn("warning");
        error("error");
    }

    /// 로그는 영문으로만 남긴다. 사용자가 보내 준 로그를 누구나 읽을 수 있어야 하고,
    /// 편집기가 UTF-8로 열지 않아도 깨지지 않는다.
    ///
    /// 로그 문구가 `{error}`·`{note}`로 다른 문자열을 끌어오므로 로그 호출만이 아니라
    /// 소스의 문자열 리터럴 전부(주석과 테스트 제외)를 검사한다.
    #[test]
    fn source_strings_are_english() {
        const SOURCES: [(&str, &str); 22] = [
            ("app.rs", include_str!("app.rs")),
            ("config.rs", include_str!("config.rs")),
            ("devtools.rs", include_str!("devtools.rs")),
            ("ipc.rs", include_str!("ipc.rs")),
            ("logging.rs", include_str!("logging.rs")),
            ("main.rs", include_str!("main.rs")),
            ("platform.rs", include_str!("platform.rs")),
            ("runtime.rs", include_str!("runtime.rs")),
            ("startup.rs", include_str!("startup.rs")),
            ("theme.rs", include_str!("theme.rs")),
            ("timer.rs", include_str!("timer.rs")),
            ("win32.rs", include_str!("win32.rs")),
            ("icon/cache.rs", include_str!("icon/cache.rs")),
            ("icon/layout.rs", include_str!("icon/layout.rs")),
            ("icon/mod.rs", include_str!("icon/mod.rs")),
            ("icon/render.rs", include_str!("icon/render.rs")),
            ("metrics/format.rs", include_str!("metrics/format.rs")),
            ("metrics/network.rs", include_str!("metrics/network.rs")),
            ("settings/dialog.rs", include_str!("settings/dialog.rs")),
            ("settings/model.rs", include_str!("settings/model.rs")),
            ("tray/mod.rs", include_str!("tray/mod.rs")),
            ("tray/onboard.rs", include_str!("tray/onboard.rs")),
        ];
        for (name, source) in SOURCES {
            for literal in string_literals(source) {
                assert!(
                    !literal.chars().any(is_hangul),
                    "{name}: 로그로 갈 수 있는 문자열은 영문이어야 한다: {literal}"
                );
            }
        }
    }

    fn is_hangul(c: char) -> bool {
        matches!(c, '\u{AC00}'..='\u{D7A3}' | '\u{3131}'..='\u{318E}')
    }

    /// 주석과 `#[cfg(test)]` 아래를 뺀 문자열 리터럴. 문자 리터럴(`'"'`)은 건너뛴다.
    fn string_literals(source: &str) -> Vec<String> {
        let code = source.split("#[cfg(test)]").next().unwrap_or(source);
        let chars: Vec<char> = code.chars().collect();
        let mut literals = Vec::new();
        let mut i = 0;
        while i < chars.len() {
            match chars[i] {
                '/' if chars.get(i + 1) == Some(&'/') => {
                    while i < chars.len() && chars[i] != '\n' {
                        i += 1;
                    }
                }
                '\'' if chars.get(i + 2) == Some(&'\'') => i += 3,
                '\'' if chars.get(i + 1) == Some(&'\\') && chars.get(i + 3) == Some(&'\'') => {
                    i += 4
                }
                // 원시 문자열 `r"..."`은 `\`가 이스케이프가 아니다.
                'r' if chars.get(i + 1) == Some(&'"')
                    && !i
                        .checked_sub(1)
                        .is_some_and(|before| chars[before].is_alphanumeric()) =>
                {
                    let start = i + 2;
                    i = start;
                    while i < chars.len() && chars[i] != '"' {
                        i += 1;
                    }
                    literals.push(chars[start..i.min(chars.len())].iter().collect());
                    i += 1;
                }
                '"' => {
                    let start = i + 1;
                    i += 1;
                    while i < chars.len() && chars[i] != '"' {
                        i += if chars[i] == '\\' { 2 } else { 1 };
                    }
                    literals.push(chars[start..i.min(chars.len())].iter().collect());
                    i += 1;
                }
                _ => i += 1,
            }
        }
        literals
    }

    #[test]
    fn string_literal_scanner_skips_comments_and_chars() {
        let source = "// \"주석\"\nlet a = '\"'; let b = \"ok\"; let c = \"\\\"x\"; let d = r\"a\\\"; for x in y {}";
        assert_eq!(string_literals(source), vec!["ok", "\\\"x", "a\\"]);
    }

    #[test]
    fn log_line_has_level_and_time() {
        let line = format!("[{}] {:<5} {}", timestamp(), "INFO", "hello");
        assert!(line.contains("INFO"));
        assert!(line.contains("hello"));
    }
}
