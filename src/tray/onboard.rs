//! 첫 실행 안내와 트레이 승격 감지.
//!
//! 새로 등록한 트레이 아이콘은 Windows가 오버플로우(`^`) 안에 숨긴다.
//! 사용자가 한 번 승격시켜야 트레이에 보이므로, 보이지 않으면 안내해야 한다.
//!
//! 승격 여부는 `HKCU\Control Panel\NotifyIconSettings\<hash>\IsPromoted`로 판단한다.
//! 이 키는 실행 경로별로 만들어지고 툴팁 변경에는 영향받지 않는다(실측).

use windows_registry::CURRENT_USER;

use crate::config::State;
use crate::win32;

const NOTIFY_KEY: &str = r"Control Panel\NotifyIconSettings";
const SETTINGS_URI: &str = "ms-settings:taskbar";

/// 트레이 아이콘이 트레이에 보이는 상태.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Promotion {
    /// 등록한 아이콘이 모두 트레이에 보인다.
    All,
    /// 일부만 보인다.
    Some { promoted: usize, total: usize },
    /// 이 실행 파일로 등록된 아이콘 키가 아직 없다.
    None,
    /// 레지스트리를 읽지 못했다.
    Unknown,
}

impl Promotion {
    pub fn summary(self) -> String {
        match self {
            Promotion::All => "the readouts are visible in the taskbar".to_string(),
            Promotion::Some { promoted, total } if promoted > 0 => format!(
                "{promoted} of {total} readouts are visible; the rest are hidden in the overflow area"
            ),
            // 아직 트레이 항목이 없거나 모두 숨겨진 경우다. 사용자가 해야 할 일은 같다.
            Promotion::Some { .. } | Promotion::None => {
                "the readouts are hidden in the taskbar overflow area".to_string()
            }
            Promotion::Unknown => "the tray settings could not be read".to_string(),
        }
    }
}

fn current_exe() -> Option<String> {
    std::env::current_exe()
        .ok()
        .map(|path| path.display().to_string())
}

/// `\WindowsApps\` 뒤의 경로. MSIX 앱의 실행 경로는 설치 버전마다 바뀐다.
fn after_windows_apps(path: &str) -> Option<&str> {
    // ASCII만 소문자로 바꾸므로 비ASCII 사용자 이름이 있어도 인덱스가 유지된다.
    let lower = path.to_ascii_lowercase();
    let index = lower.find(r"\WindowsApps\".to_ascii_lowercase().as_str())?;
    Some(&path[index + 1..])
}

/// 레지스트리에 적힌 실행 경로와 현재 exe가 같은 프로그램인가.
///
/// 패키지 앱은 `{6D809377-…}\WindowsApps\…`처럼 known-folder GUID를 붙여 적는다.
/// 전체 문자열을 비교하면 항상 불일치하므로 `WindowsApps` 이후만 비교한다.
pub fn same_executable(registered: &str, current: &str) -> bool {
    if registered.eq_ignore_ascii_case(current) {
        return true;
    }
    match (after_windows_apps(registered), after_windows_apps(current)) {
        (Some(left), Some(right)) => left.eq_ignore_ascii_case(right),
        _ => false,
    }
}

/// `NotifyIconSettings`가 쓰는 GUID 문자열(`{XXXXXXXX-XXXX-...}`)로 만든다.
fn guid_text(guid: u128) -> String {
    let hex = format!("{guid:032X}");
    format!(
        "{{{}-{}-{}-{}-{}}}",
        &hex[0..8],
        &hex[8..12],
        &hex[12..16],
        &hex[16..20],
        &hex[20..32]
    )
}

/// 주어진 고정 GUID가 모두 `NotifyIconSettings`에 등록되어 있는가.
///
/// Explorer가 아이콘을 등록할 때 이 키를 만든다. 키를 읽지 못하면 확인할 수
/// 없으므로 `true`를 돌려주고, 호출자가 GUID를 포기하지 않게 한다.
pub fn guids_present(guids: &[u128; 4]) -> bool {
    let Ok(key) = CURRENT_USER.open(NOTIFY_KEY) else {
        return true;
    };
    let Ok(names) = key.keys() else {
        return true;
    };
    let wanted: Vec<String> = guids.iter().map(|guid| guid_text(*guid)).collect();
    let mut seen = vec![false; wanted.len()];
    for name in names {
        let Ok(entry) = key.open(&name) else {
            continue;
        };
        let Ok(value) = entry.get_string("IconGuid") else {
            continue;
        };
        for (index, guid) in wanted.iter().enumerate() {
            if !seen[index] && value.eq_ignore_ascii_case(guid) {
                seen[index] = true;
            }
        }
    }
    seen.iter().all(|matched| *matched)
}

/// 이 실행 파일로 등록된 아이콘들의 승격 상태.
pub fn promotion_state() -> Promotion {
    let Some(exe) = current_exe() else {
        return Promotion::Unknown;
    };
    let Ok(key) = CURRENT_USER.open(NOTIFY_KEY) else {
        return Promotion::Unknown;
    };
    let Ok(names) = key.keys() else {
        return Promotion::Unknown;
    };

    let mut total = 0usize;
    let mut promoted = 0usize;
    for name in names {
        let Ok(entry) = key.open(&name) else {
            continue;
        };
        let Ok(path) = entry.get_string("ExecutablePath") else {
            continue;
        };
        if !same_executable(&path, &exe) {
            continue;
        }
        total += 1;
        if entry.get_u32("IsPromoted").unwrap_or(0) == 1 {
            promoted += 1;
        }
    }

    match total {
        0 => Promotion::None,
        total if promoted == total => Promotion::All,
        total => Promotion::Some { promoted, total },
    }
}

/// 첫 실행이거나, 아이콘이 숨겨진 채로 버전이 바뀌었으면 안내한다.
///
/// 모두 보이면 알릴 것이 없으므로 건너뛴다. 같은 버전에서는 한 번만 안내한다.
/// (사용자가 특정 아이콘을 트레이에 두지 않기로 한 선택을 매번 되묻지 않기 위해서다.)
pub fn should_onboard(state: &State, promotion: Promotion) -> bool {
    if promotion == Promotion::Unknown || promotion == Promotion::All {
        return false;
    }
    !state.onboarding_shown || state.last_version != env!("CARGO_PKG_VERSION")
}

fn onboarding_text(promotion: Promotion) -> String {
    let name = crate::PRODUCT_NAME;
    format!(
        "{name} is running.\n\n\
         Windows hides new tray icons in the overflow area by default, so the\n\
         CPU / Memory / Network readouts may not be visible in the taskbar yet.\n\n\
         Current state: {}.\n\n\
         Open the taskbar settings and turn on {name} under\n\
         \"Other system tray icons\"?",
        promotion.summary()
    )
}

/// 안내 대화상자를 띄운다. 사용자가 수락하면 작업 표시줄 설정을 연다.
pub fn show_onboarding(promotion: Promotion) -> bool {
    if !win32::confirm(crate::PRODUCT_NAME, &onboarding_text(promotion)) {
        return false;
    }
    open_taskbar_settings()
}

/// Windows 작업 표시줄 설정 열기.
pub fn open_taskbar_settings() -> bool {
    win32::open_url(SETTINGS_URI)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn promotion_state_is_readable() {
        // 어떤 값이든 읽히면 된다(환경에 따라 None/Some/All).
        let state = promotion_state();
        assert!(
            matches!(
                state,
                Promotion::All | Promotion::Some { .. } | Promotion::None | Promotion::Unknown
            ),
            "{state:?}"
        );
    }

    #[test]
    fn guid_text_matches_the_registry_format() {
        assert_eq!(
            guid_text(0xf2ac86269de9ee99441a1aef4c9406a1),
            "{F2AC8626-9DE9-EE99-441A-1AEF4C9406A1}"
        );
    }

    #[test]
    fn unknown_promotion_never_onboards() {
        let fresh = State::default();
        assert!(!should_onboard(&fresh, Promotion::Unknown));
    }

    #[test]
    fn first_run_onboards_once_per_version() {
        let mut state = State::default();
        assert!(
            should_onboard(&state, Promotion::None),
            "첫 실행이면 안내한다"
        );

        state.onboarding_shown = true;
        state.last_version = env!("CARGO_PKG_VERSION").to_string();
        assert!(
            !should_onboard(&state, Promotion::None),
            "같은 버전에서는 다시 안내하지 않는다"
        );

        state.last_version = "0.0.0".to_string();
        assert!(
            should_onboard(&state, Promotion::None),
            "버전이 바뀌면 다시 안내한다"
        );
    }

    #[test]
    fn text_mentions_the_current_state() {
        let text = onboarding_text(Promotion::Some {
            promoted: 1,
            total: 3,
        });
        assert!(text.contains("1 of 3"));
        assert!(text.contains("Other system tray icons"));
    }

    #[test]
    fn hidden_states_say_where_to_look() {
        for state in [
            Promotion::None,
            Promotion::Some {
                promoted: 0,
                total: 3,
            },
        ] {
            let summary = state.summary();
            assert!(summary.contains("overflow"), "{summary}");
            assert!(!summary.contains("registered"), "{summary}");
        }
        assert!(Promotion::All.summary().contains("visible"));
    }

    /// 패키지 앱은 `{GUID}\WindowsApps\…` 형식으로 경로를 적는다.
    #[test]
    fn packaged_paths_are_compared_after_windows_apps() {
        let registered = r"{6D809377-6AF0-444B-8957-A3773F02200E}\WindowsApps\Tinyrack.TrayMonitor_0.3.1.0_x64__abc\tray-monitor.exe";
        let current =
            r"C:\Program Files\WindowsApps\Tinyrack.TrayMonitor_0.3.1.0_x64__abc\tray-monitor.exe";
        assert!(same_executable(registered, current));

        let other =
            r"C:\Program Files\WindowsApps\Tinyrack.TrayMonitor_0.2.0.0_x64__abc\tray-monitor.exe";
        assert!(
            !same_executable(registered, other),
            "다른 패키지 버전의 경로는 다르다"
        );

        // 대소문자는 Windows 경로처럼 무시한다.
        assert!(same_executable(registered, &current.to_uppercase()));
    }

    #[test]
    fn unpackaged_paths_still_match() {
        let path = r"C:\Users\me\AppData\Local\Programs\tray-monitor\tray-monitor.exe";
        assert!(same_executable(path, &path.to_uppercase()));
        assert!(!same_executable(path, r"C:\Other\tray-monitor.exe"));
    }
}
