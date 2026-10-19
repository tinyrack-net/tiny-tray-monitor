//! 물리 메모리 사용량. `GlobalMemoryStatusEx`만 사용한다.

use core::mem::size_of;

use windows::Win32::System::SystemInformation::{GlobalMemoryStatusEx, MEMORYSTATUSEX};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Memory {
    pub percent: f64,
    pub used: u64,
    pub total: u64,
}

/// 사용률 계산. 순수 함수라 테스트로 검증한다.
pub fn percent(total: u64, avail: u64) -> f64 {
    if total == 0 {
        return 0.0;
    }
    let used = total.saturating_sub(avail.min(total));
    (used as f64 / total as f64 * 100.0).clamp(0.0, 100.0)
}

pub fn read() -> Option<Memory> {
    let mut status = MEMORYSTATUSEX {
        dwLength: size_of::<MEMORYSTATUSEX>() as u32,
        ..Default::default()
    };
    unsafe { GlobalMemoryStatusEx(&mut status) }.ok()?;
    let total = status.ullTotalPhys;
    let avail = status.ullAvailPhys.min(total);
    Some(Memory {
        percent: percent(total, avail),
        used: total.saturating_sub(avail),
        total,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn percent_basics() {
        assert_eq!(percent(0, 0), 0.0);
        assert_eq!(percent(16, 4), 75.0);
        assert_eq!(percent(16, 16), 0.0);
        assert_eq!(percent(16, 0), 100.0);
    }

    #[test]
    fn avail_larger_than_total_is_clamped() {
        assert_eq!(percent(16, 32), 0.0);
    }

    #[test]
    fn read_returns_plausible_values() {
        let m = read().expect("GlobalMemoryStatusEx");
        assert!(m.total > 0);
        assert!(m.used <= m.total);
        assert!((0.0..=100.0).contains(&m.percent));
    }
}
