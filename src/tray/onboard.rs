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
        if !path.eq_ignore_ascii_case(&exe) {
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

/// 첫 실행이거나 버전이 바뀌었으면 안내한다.
pub fn should_onboard(state: &State, promotion: Promotion) -> bool {
    if promotion == Promotion::Unknown {
        return false;
    }
    !state.onboarding_shown || state.last_version != env!("CARGO_PKG_VERSION")
}

fn onboarding_text(promotion: Promotion) -> String {
    format!(
        "tray-monitor is running.\n\n\
         Windows hides new tray icons in the overflow area by default, so the\n\
         CPU / Memory / Network readouts may not be visible in the taskbar yet.\n\n\
         Current state: {}.\n\n\
         Open the taskbar settings and turn on tray-monitor under\n\
         \"Other system tray icons\"?",
        promotion.summary()
    )
}

/// 안내 대화상자를 띄운다. 사용자가 수락하면 작업 표시줄 설정을 연다.
pub fn show_onboarding(promotion: Promotion) -> bool {
    if !win32::confirm("tray-monitor", &onboarding_text(promotion)) {
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
}
