//! 패키지(MSIX) 인식.
//!
//! 패키지 앱은 `%APPDATA%`/`%LOCALAPPDATA%` 쓰기가 가상 저장소로 리다이렉트된다.
//! 설정 파일 위치가 설치 이력에 따라 달라지지 않도록, 패키지에서는 `LocalState`를
//! 직접 쓴다. 자세한 실측은 docs/PACKAGING.md 참고.

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use windows::Win32::Foundation::WIN32_ERROR;
use windows::Win32::Storage::Packaging::Appx::{
    GetCurrentPackageFamilyName, GetCurrentPackageFullName,
};
use windows::core::PWSTR;

/// 첫 호출이 요구하는 버퍼 크기를 알려 줄 때의 오류 코드.
const INSUFFICIENT_BUFFER: u32 = 122;

static PACKAGED: OnceLock<bool> = OnceLock::new();
static FAMILY_NAME: OnceLock<Option<String>> = OnceLock::new();

/// 이 프로세스가 MSIX 패키지 안에서 실행 중인가.
pub fn packaged() -> bool {
    *PACKAGED.get_or_init(|| !query(GetCurrentPackageFullName).is_empty())
}

/// 패키지 패밀리 이름(`Name_PublisherId`). 패키지 밖이면 `None`.
pub fn package_family_name() -> Option<String> {
    FAMILY_NAME
        .get_or_init(|| {
            let name = query(GetCurrentPackageFamilyName);
            (!name.is_empty()).then_some(name)
        })
        .clone()
}

/// 설정 파일 디렉터리. 패키지면 `LocalState`, 아니면 `%APPDATA%\tray-monitor`.
pub fn config_dir() -> PathBuf {
    let (config, _) = base_dirs(
        packaged(),
        package_family_name().as_deref(),
        env("APPDATA").as_deref(),
        env("LOCALAPPDATA").as_deref(),
        &std::env::temp_dir(),
    );
    config
}

/// 로그 디렉터리. 패키지면 `LocalState`, 아니면 `%LOCALAPPDATA%\tray-monitor`.
pub fn data_dir() -> PathBuf {
    let (_, data) = base_dirs(
        packaged(),
        package_family_name().as_deref(),
        env("APPDATA").as_deref(),
        env("LOCALAPPDATA").as_deref(),
        &std::env::temp_dir(),
    );
    data
}

/// 패키지 첫 실행에서 예전(비패키지) 설정을 `LocalState`로 한 번 복사한다.
///
/// 패키지 앱의 `%APPDATA%` 읽기는 가상화된 `LocalCache\Roaming`을 볼 수 있는데,
/// 그곳은 이전 MSIX 빌드가 남긴 설정이므로 복사 대상으로 맞다.
pub fn migrate_legacy_data() {
    if !packaged() {
        return;
    }
    let Some(target) = local_state_dir() else {
        return;
    };
    let Some(legacy) = env("APPDATA").map(|base| base.join("tray-monitor")) else {
        return;
    };
    if legacy == target {
        return;
    }

    for name in ["config.toml", "state.toml"] {
        let source = legacy.join(name);
        let destination = target.join(name);
        if destination.exists() || !source.exists() {
            continue;
        }
        if std::fs::create_dir_all(&target).is_err() {
            continue;
        }
        if std::fs::copy(&source, &destination).is_ok() {
            crate::logging::info(format!(
                "기존 설정을 {} 에서 {} 로 복사했다",
                source.display(),
                destination.display()
            ));
        }
    }
}

fn env(name: &str) -> Option<PathBuf> {
    std::env::var_os(name).map(PathBuf::from)
}

fn local_state_dir() -> Option<PathBuf> {
    let family = package_family_name()?;
    let base = env("LOCALAPPDATA")?;
    Some(base.join("Packages").join(family).join("LocalState"))
}

/// 순수 경로 결정. 테스트가 환경 변수 없이 검증할 수 있게 분리했다.
fn base_dirs(
    packaged: bool,
    family: Option<&str>,
    appdata: Option<&Path>,
    localappdata: Option<&Path>,
    temporary: &Path,
) -> (PathBuf, PathBuf) {
    if packaged {
        if let (Some(family), Some(local)) = (family, localappdata) {
            let state = local.join("Packages").join(family).join("LocalState");
            return (state.clone(), state);
        }
    }

    let config = appdata
        .or(localappdata)
        .unwrap_or(temporary)
        .join("tray-monitor");
    let data = localappdata.unwrap_or(temporary).join("tray-monitor");
    (config, data)
}

/// 패키지 API를 두 번 호출해 이름을 얻는다. 패키지 밖이면 빈 문자열.
fn query(call: unsafe fn(*mut u32, Option<PWSTR>) -> WIN32_ERROR) -> String {
    let mut length = 0u32;
    let first = unsafe { call(&mut length, None) };
    if first.0 != INSUFFICIENT_BUFFER || length == 0 {
        return String::new();
    }

    let mut buffer = vec![0u16; length as usize];
    let second = unsafe { call(&mut length, Some(PWSTR(buffer.as_mut_ptr()))) };
    if second.0 != 0 {
        return String::new();
    }

    let end = buffer.iter().position(|unit| *unit == 0).unwrap_or(0);
    String::from_utf16_lossy(&buffer[..end])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unpackaged_keeps_the_old_locations() {
        let (config, data) = base_dirs(
            false,
            Some("Tinyrack.TrayMonitor_abc"),
            Some(Path::new(r"C:\Users\me\AppData\Roaming")),
            Some(Path::new(r"C:\Users\me\AppData\Local")),
            Path::new(r"C:\Temp"),
        );
        assert_eq!(
            config,
            PathBuf::from(r"C:\Users\me\AppData\Roaming\tray-monitor")
        );
        assert_eq!(
            data,
            PathBuf::from(r"C:\Users\me\AppData\Local\tray-monitor")
        );
    }

    #[test]
    fn packaged_uses_local_state_for_both() {
        let (config, data) = base_dirs(
            true,
            Some("Tinyrack.TrayMonitor_abc"),
            Some(Path::new(r"C:\Users\me\AppData\Roaming")),
            Some(Path::new(r"C:\Users\me\AppData\Local")),
            Path::new(r"C:\Temp"),
        );
        let state = PathBuf::from(
            r"C:\Users\me\AppData\Local\Packages\Tinyrack.TrayMonitor_abc\LocalState",
        );
        assert_eq!(config, state);
        assert_eq!(data, state);
    }

    #[test]
    fn packaged_without_family_name_falls_back() {
        let (config, data) = base_dirs(
            true,
            None,
            Some(Path::new(r"C:\Roaming")),
            Some(Path::new(r"C:\Local")),
            Path::new(r"C:\Temp"),
        );
        assert_eq!(config, PathBuf::from(r"C:\Roaming\tray-monitor"));
        assert_eq!(data, PathBuf::from(r"C:\Local\tray-monitor"));
    }

    #[test]
    fn missing_appdata_falls_back_to_local_then_temp() {
        let (config, data) = base_dirs(
            false,
            None,
            None,
            Some(Path::new(r"C:\Local")),
            Path::new(r"C:\Temp"),
        );
        assert_eq!(config, PathBuf::from(r"C:\Local\tray-monitor"));
        assert_eq!(data, PathBuf::from(r"C:\Local\tray-monitor"));

        let (config, data) = base_dirs(false, None, None, None, Path::new(r"C:\Temp"));
        assert_eq!(config, PathBuf::from(r"C:\Temp\tray-monitor"));
        assert_eq!(data, PathBuf::from(r"C:\Temp\tray-monitor"));
    }

    /// 이 테스트 프로세스는 패키지 밖에서 돈다.
    #[test]
    fn this_test_runs_unpackaged() {
        assert!(!packaged());
        assert_eq!(package_family_name(), None);
    }
}
