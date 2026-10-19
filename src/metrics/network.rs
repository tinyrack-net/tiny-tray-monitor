//! 네트워크 처리량. `GetIfTable2`의 인터페이스별 누적 바이트 델타로 계산한다.
//!
//! 판정 로직은 순수 함수로 떼어내 테스트한다.

use core::ffi::c_void;
use std::time::Instant;

use windows::Win32::NetworkManagement::IpHelper::{
    FreeMibTable, GetIfTable2, MIB_IF_ROW2, MIB_IF_TABLE2,
};

/// `IF_TYPE_SOFTWARE_LOOPBACK`
const IF_TYPE_SOFTWARE_LOOPBACK: u32 = 24;
/// `IF_TYPE_TUNNEL`
const IF_TYPE_TUNNEL: u32 = 131;
/// `MIB_IF_OPER_STATUS_UP`
const OPER_STATUS_UP: i32 = 1;
/// `MIB_IF_ROW2::InterfaceAndOperStatusFlags`의 HardwareInterface 비트.
const FLAG_HARDWARE_INTERFACE: u8 = 0x01;

#[derive(Debug, Clone)]
pub struct Adapter {
    pub index: u32,
    pub alias: String,
    pub description: String,
    pub if_type: u32,
    pub up: bool,
    pub hardware: bool,
    /// 이 인터페이스가 집계에 포함되는지. `--interfaces` 진단과 같은 판정을 쓴다.
    pub counted: bool,
    pub rx: u64,
    pub tx: u64,
}

fn utf16_string(buf: &[u16]) -> String {
    let end = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    String::from_utf16_lossy(&buf[..end]).trim().to_string()
}

/// 트래픽 집계 대상이 될 수 있는 타입인지. 루프백과 터널은 제외한다.
pub fn is_traffic_type(if_type: u32) -> bool {
    if_type != IF_TYPE_SOFTWARE_LOOPBACK && if_type != IF_TYPE_TUNNEL
}

/// 집계 대상 판정.
///
/// `exclude_virtual`이면 하드웨어 인터페이스만 센다. 가상 스위치와 물리 NIC가
/// 같은 트래픽을 두 번 세는 것을 막기 위함이다. 다만 물리 인터페이스가 하나도
/// 활성 상태가 아니면(예: VPN 전용 환경) 가상 인터페이스라도 세야 값이 나온다.
pub fn is_counted(if_type: u32, up: bool, hardware: bool, exclude_virtual: bool) -> bool {
    up && is_traffic_type(if_type) && (!exclude_virtual || hardware)
}

fn read_adapters() -> Option<Vec<Adapter>> {
    unsafe {
        let mut table: *mut MIB_IF_TABLE2 = std::ptr::null_mut();
        if GetIfTable2(&mut table).is_err() || table.is_null() {
            return None;
        }
        let entries =
            std::slice::from_raw_parts((*table).Table.as_ptr(), (*table).NumEntries as usize);
        let list = entries.iter().map(|row| adapter_from_row(row)).collect();
        FreeMibTable(table as *const c_void);
        Some(list)
    }
}

unsafe fn adapter_from_row(row: &MIB_IF_ROW2) -> Adapter {
    let hardware = row.InterfaceAndOperStatusFlags._bitfield & FLAG_HARDWARE_INTERFACE != 0;
    Adapter {
        index: row.InterfaceIndex,
        alias: utf16_string(&row.Alias),
        description: utf16_string(&row.Description),
        if_type: row.Type,
        up: row.OperStatus.0 == OPER_STATUS_UP,
        hardware,
        counted: false,
        rx: row.InOctets,
        tx: row.OutOctets,
    }
}

/// 인터페이스 목록과 `counted` 판정을 함께 만든다.
fn collect(exclude_virtual: bool) -> Option<Vec<Adapter>> {
    let mut list = read_adapters()?;
    let hardware_up = list
        .iter()
        .filter(|a| a.up && a.hardware && is_traffic_type(a.if_type))
        .count();
    let use_filter = exclude_virtual && hardware_up > 0;
    for a in &mut list {
        a.counted = is_counted(a.if_type, a.up, a.hardware, use_filter);
    }
    list.sort_by_key(|a| a.index);
    Some(list)
}

/// 진단용 인터페이스 목록.
pub fn adapters(exclude_virtual: bool) -> Vec<Adapter> {
    collect(exclude_virtual).unwrap_or_default()
}

/// 집계 대상으로 표시된 인터페이스의 누적 (수신, 송신) 바이트를 더한다.
///
/// 순수 함수라 \`--interfaces\`가 보여주는 목록과 정확히 같은 판정을 쓰는지 테스트할 수 있다.
pub fn sum_counted(list: &[Adapter]) -> (u64, u64) {
    let mut rx = 0u64;
    let mut tx = 0u64;
    for a in list.iter().filter(|a| a.counted) {
        rx = rx.saturating_add(a.rx);
        tx = tx.saturating_add(a.tx);
    }
    (rx, tx)
}

/// 집계 대상 인터페이스의 누적 (수신, 송신) 바이트.
pub fn totals(exclude_virtual: bool) -> Option<(u64, u64)> {
    let list = collect(exclude_virtual)?;
    Some(sum_counted(&list))
}

/// 델타와 경과 시간으로 초당 바이트를 구한다.
pub fn rate(delta_bytes: u64, seconds: f64) -> f64 {
    if !seconds.is_finite() || seconds <= 0.0 {
        return 0.0;
    }
    let v = delta_bytes as f64 / seconds;
    if v.is_finite() { v.max(0.0) } else { 0.0 }
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Rate {
    pub down: f64,
    pub up: f64,
}

#[derive(Debug)]
pub struct NetMeter {
    exclude_virtual: bool,
    prev: Option<(u64, u64, Instant)>,
}

impl NetMeter {
    pub fn new(exclude_virtual: bool) -> Self {
        Self {
            exclude_virtual,
            prev: None,
        }
    }

    /// 첫 호출은 기준점만 잡고 `None`을 돌려준다.
    pub fn sample(&mut self) -> Option<Rate> {
        let (rx, tx) = totals(self.exclude_virtual)?;
        let now = Instant::now();
        let (prev_rx, prev_tx, prev_at) = self.prev.replace((rx, tx, now))?;
        let seconds = now.duration_since(prev_at).as_secs_f64();
        Some(Rate {
            down: rate(rx.saturating_sub(prev_rx), seconds),
            up: rate(tx.saturating_sub(prev_tx), seconds),
        })
    }

    pub fn reset(&mut self) {
        self.prev = None;
    }

    pub fn set_exclude_virtual(&mut self, exclude: bool) {
        if self.exclude_virtual != exclude {
            self.exclude_virtual = exclude;
            self.reset();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loopback_and_tunnel_are_never_counted() {
        for exclude_virtual in [true, false] {
            assert!(!is_counted(
                IF_TYPE_SOFTWARE_LOOPBACK,
                true,
                true,
                exclude_virtual
            ));
            assert!(!is_counted(IF_TYPE_TUNNEL, true, true, exclude_virtual));
        }
    }

    #[test]
    fn down_interfaces_are_never_counted() {
        assert!(!is_counted(6, false, true, false));
    }

    #[test]
    fn virtual_interfaces_follow_the_flag() {
        assert!(!is_counted(6, true, false, true));
        assert!(is_counted(6, true, false, false));
        assert!(is_counted(6, true, true, true));
    }

    #[test]
    fn rate_guards_against_bad_time() {
        assert_eq!(rate(1024, 0.0), 0.0);
        assert_eq!(rate(1024, -1.0), 0.0);
        assert_eq!(rate(1024, f64::NAN), 0.0);
        assert_eq!(rate(2048, 2.0), 1024.0);
    }

    #[test]
    fn totals_are_available_on_this_machine() {
        assert!(totals(true).is_some());
    }

    fn adapter(index: u32, counted: bool, rx: u64, tx: u64) -> Adapter {
        Adapter {
            index,
            alias: String::new(),
            description: String::new(),
            if_type: 6,
            up: true,
            hardware: true,
            counted,
            rx,
            tx,
        }
    }

    #[test]
    fn sum_counted_ignores_uncounted_adapters() {
        let list = vec![
            adapter(1, true, 100, 10),
            adapter(2, false, 999, 999),
            adapter(3, true, 1, 2),
        ];
        assert_eq!(sum_counted(&list), (101, 12));
    }

    #[test]
    fn counted_bytes_never_exceed_total_bytes() {
        let list = collect(true).expect("collect");
        let all_rx: u64 = list.iter().map(|a| a.rx).sum();
        let all_tx: u64 = list.iter().map(|a| a.tx).sum();
        let (rx, tx) = sum_counted(&list);
        assert!(rx <= all_rx && tx <= all_tx);
    }

    #[test]
    fn counted_flags_match_the_is_counted_rule() {
        let list = collect(true).expect("collect");
        let hardware_up = list
            .iter()
            .filter(|a| a.up && a.hardware && is_traffic_type(a.if_type))
            .count();
        for a in &list {
            let expected = is_counted(a.if_type, a.up, a.hardware, hardware_up > 0);
            assert_eq!(a.counted, expected, "index {}: {}", a.index, a.description);
        }
    }

    #[test]
    fn meter_needs_two_samples() {
        let mut meter = NetMeter::new(true);
        assert!(meter.sample().is_none());
        let rate = meter.sample().expect("두 번째 샘플");
        assert!(rate.down >= 0.0 && rate.up >= 0.0);
    }
}
