//! CPU 사용률. `GetSystemTimes`의 누적 시간 델타로 계산한다.
//!
//! 관리자 권한이나 성능 카운터 라이브러리가 필요 없다.

use windows::Win32::Foundation::FILETIME;
use windows::Win32::System::Threading::GetSystemTimes;

/// 누적 CPU 시간(100ns 단위).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Times {
    pub idle: u64,
    pub kernel: u64,
    pub user: u64,
}

fn filetime_to_u64(ft: FILETIME) -> u64 {
    ((ft.dwHighDateTime as u64) << 32) | ft.dwLowDateTime as u64
}

/// 현재 누적 시간을 읽는다.
pub fn read() -> Option<Times> {
    let (mut idle, mut kernel, mut user) = (
        FILETIME::default(),
        FILETIME::default(),
        FILETIME::default(),
    );
    unsafe { GetSystemTimes(Some(&mut idle), Some(&mut kernel), Some(&mut user)) }.ok()?;
    Some(Times {
        idle: filetime_to_u64(idle),
        kernel: filetime_to_u64(kernel),
        user: filetime_to_u64(user),
    })
}

/// 두 샘플 사이의 CPU 사용률(%). 커널 시간에는 유휴 시간이 포함된다.
///
/// 델타가 0이거나(샘플 간격이 너무 짧음) 카운터가 역행하면 `None`을 돌려준다.
pub fn usage(prev: Times, cur: Times) -> Option<f64> {
    let idle = cur.idle.saturating_sub(prev.idle);
    let kernel = cur.kernel.saturating_sub(prev.kernel);
    let user = cur.user.saturating_sub(prev.user);
    let total = kernel.saturating_add(user);
    if total == 0 {
        return None;
    }
    let busy = total.saturating_sub(idle).min(total);
    let value = busy as f64 / total as f64 * 100.0;
    value.is_finite().then(|| value.clamp(0.0, 100.0))
}

/// 직전 샘플을 보관하며 사용률을 계산한다.
#[derive(Debug, Default)]
pub struct CpuMeter {
    prev: Option<Times>,
}

impl CpuMeter {
    pub fn new() -> Self {
        Self { prev: read() }
    }

    /// 직전 호출 이후 구간의 사용률. 첫 호출(또는 델타 0)이면 `None`.
    pub fn sample(&mut self) -> Option<f64> {
        let cur = read()?;
        let prev = self.prev.replace(cur)?;
        usage(prev, cur)
    }

    /// 절전 복귀처럼 긴 공백 뒤에 호출한다. 다음 샘플이 0이나 100으로 튀지 않게 한다.
    pub fn reset(&mut self) {
        self.prev = read();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn times(idle: u64, kernel: u64, user: u64) -> Times {
        Times { idle, kernel, user }
    }

    #[test]
    fn half_busy() {
        // total 100, idle 50 -> 50%
        assert_eq!(usage(times(0, 50, 50), times(50, 100, 100)), Some(50.0));
    }

    #[test]
    fn fully_idle() {
        // 커널 델타에 유휴가 포함되므로 idle == kernel 이면 0%
        assert_eq!(usage(times(0, 0, 0), times(100, 100, 0)), Some(0.0));
    }

    #[test]
    fn zero_delta_is_rejected() {
        assert_eq!(usage(times(1, 2, 3), times(1, 2, 3)), None);
    }

    #[test]
    fn counter_going_backwards_is_rejected() {
        assert_eq!(usage(times(500, 900, 900), times(0, 0, 0)), None);
    }

    #[test]
    fn result_is_within_range() {
        for idle in [0u64, 1, 50, 99, 1000] {
            let v = usage(times(0, 0, 0), times(idle, 100, 100)).unwrap();
            assert!((0.0..=100.0).contains(&v), "{v}");
        }
    }

    #[test]
    fn meter_needs_two_samples() {
        let mut meter = CpuMeter::new();
        assert!(meter.sample().is_none(), "첫 샘플은 델타가 없다");
    }
}
