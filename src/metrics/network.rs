//! 네트워크 처리량. `GetIfTable2`의 인터페이스별 누적 바이트 델타로 계산한다.
//!
//! 기본은 집계 대상 인터페이스 전부의 합이고, 설정에서 인터페이스 하나를 고르면 그것만
//! 잰다. 고른 인터페이스는 별칭(`Ethernet`, `Wi-Fi`)으로 찾는다. 판정 로직은 순수 함수로
//! 떼어내 테스트한다.

use core::ffi::c_void;
use std::time::Instant;

use windows::Win32::NetworkManagement::IpHelper::{
    FreeMibTable, GAA_FLAG_SKIP_ANYCAST, GAA_FLAG_SKIP_DNS_SERVER, GAA_FLAG_SKIP_MULTICAST,
    GAA_FLAG_SKIP_UNICAST, GetAdaptersAddresses, GetIfTable2, IP_ADAPTER_ADDRESSES_LH, MIB_IF_ROW2,
    MIB_IF_TABLE2,
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
/// `hardware_only`이면 하드웨어 인터페이스만 센다. 가상 스위치·필터 인스턴스와 물리 NIC가
/// 같은 트래픽을 두 번 세는 것을 막기 위함이다. 다만 물리 인터페이스가 하나도
/// 활성 상태가 아니면(예: VPN 전용 환경) 가상 인터페이스라도 세야 값이 나온다.
/// 가상 인터페이스만 재고 싶으면 설정에서 그 인터페이스를 고른다([select]).
pub fn is_counted(if_type: u32, up: bool, hardware: bool, hardware_only: bool) -> bool {
    up && is_traffic_type(if_type) && (!hardware_only || hardware)
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
fn collect() -> Option<Vec<Adapter>> {
    let mut list = read_adapters()?;
    let hardware_up = list
        .iter()
        .filter(|a| a.up && a.hardware && is_traffic_type(a.if_type))
        .count();
    let hardware_only = hardware_up > 0;
    for a in &mut list {
        a.counted = is_counted(a.if_type, a.up, a.hardware, hardware_only);
    }
    list.sort_by_key(|a| a.index);
    Some(list)
}

/// 진단용 인터페이스 목록.
pub fn adapters() -> Vec<Adapter> {
    collect().unwrap_or_default()
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

/// 고른 인터페이스만 집계 대상으로 남긴다. 그 별칭이 없으면 그대로 두고 `false`.
///
/// 필터 드라이버 인스턴스는 별칭 뒤에 `-WFP ...`가 붙으므로 정확히 같은 별칭은 본체뿐이다.
pub fn select(list: &mut [Adapter], alias: &str) -> bool {
    let alias = alias.trim();
    if alias.is_empty() || !list.iter().any(|a| a.alias == alias) {
        return false;
    }
    for a in list.iter_mut() {
        a.counted = a.alias == alias;
    }
    true
}

/// 집계 대상 인터페이스의 누적 (수신, 송신) 바이트와, 고른 인터페이스를 찾았는지.
pub fn totals(alias: &str) -> Option<(u64, u64, bool)> {
    let mut list = collect()?;
    let found = select(&mut list, alias);
    let (rx, tx) = sum_counted(&list);
    Some((rx, tx, found))
}

/// 설정 창의 인터페이스 목록에 쓰는 정보.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Interface {
    /// 별칭(`Ethernet`). 설정에 이 값이 저장된다.
    pub alias: String,
    /// 드라이버 설명(`Intel(R) I211 Gigabit Network Connection`).
    pub description: String,
    pub up: bool,
}

/// 사용자가 고를 만한 인터페이스. `Get-NetAdapter`와 같은 목록에서 루프백만 뺀다.
///
/// `GetIfTable2`는 WFP·QoS 필터 인스턴스와 WAN Miniport까지 돌려줘 고르기 어렵다
/// (실측: 59개 중 사람이 아는 이름은 9개). `GetAdaptersAddresses`는 그 9개만 준다.
pub fn interfaces() -> Vec<Interface> {
    let flags = GAA_FLAG_SKIP_UNICAST
        | GAA_FLAG_SKIP_ANYCAST
        | GAA_FLAG_SKIP_MULTICAST
        | GAA_FLAG_SKIP_DNS_SERVER;
    let mut size = 16 * 1024u32;
    for _ in 0..3 {
        // IP_ADAPTER_ADDRESSES는 8바이트 정렬이다.
        let mut buffer = vec![0u64; (size as usize).div_ceil(8)];
        let head = buffer.as_mut_ptr() as *mut IP_ADAPTER_ADDRESSES_LH;
        let status = unsafe { GetAdaptersAddresses(0, flags, None, Some(head), &mut size) };
        match status {
            0 => return unsafe { read_interfaces(head) },
            // ERROR_BUFFER_OVERFLOW: size에 필요한 크기가 들어 있다.
            111 => continue,
            _ => return Vec::new(),
        }
    }
    Vec::new()
}

/// `GetAdaptersAddresses`의 연결 리스트를 읽는다.
///
/// # Safety
/// `node`는 성공한 `GetAdaptersAddresses`가 채운 버퍼의 첫 항목이고, 버퍼가 살아 있어야 한다.
unsafe fn read_interfaces(mut node: *const IP_ADAPTER_ADDRESSES_LH) -> Vec<Interface> {
    let text = |p: windows::core::PWSTR| {
        if p.is_null() {
            String::new()
        } else {
            unsafe { p.to_string() }
                .unwrap_or_default()
                .trim()
                .to_string()
        }
    };
    let mut list = Vec::new();
    while let Some(entry) = unsafe { node.as_ref() } {
        if entry.IfType != IF_TYPE_SOFTWARE_LOOPBACK {
            let alias = text(entry.FriendlyName);
            if !alias.is_empty() {
                list.push(Interface {
                    alias,
                    description: text(entry.Description),
                    up: entry.OperStatus.0 == OPER_STATUS_UP,
                });
            }
        }
        node = entry.Next;
    }
    // 연결된 것을 먼저, 그 안에서는 이름 순서.
    list.sort_by(|a, b| b.up.cmp(&a.up).then_with(|| a.alias.cmp(&b.alias)));
    list
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
    /// 고른 인터페이스의 별칭. 빈 값이면 집계 대상 전부.
    device: String,
    /// 직전 표본에서 고른 인터페이스를 찾았는가. 바뀌면 합산 범위가 달라진다.
    found: bool,
    prev: Option<(u64, u64, Instant)>,
}

impl NetMeter {
    pub fn new(device: &str) -> Self {
        Self {
            device: device.trim().to_string(),
            found: false,
            prev: None,
        }
    }

    /// 지금 재는 인터페이스. 고르지 않았거나 찾지 못했으면 `None`(전부의 합).
    pub fn device(&self) -> Option<&str> {
        (self.found && !self.device.is_empty()).then_some(self.device.as_str())
    }

    /// 첫 호출은 기준점만 잡고 `None`을 돌려준다.
    pub fn sample(&mut self) -> Option<Rate> {
        let (rx, tx, found) = totals(&self.device)?;
        // 고른 인터페이스가 생기거나 사라지면 합산 범위가 바뀌어 델타가 튄다. 기준점부터 다시.
        if found != self.found {
            self.found = found;
            self.prev = None;
        }
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

    pub fn set_device(&mut self, device: &str) {
        let device = device.trim();
        if self.device != device {
            self.device = device.to_string();
            self.found = false;
            self.reset();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loopback_and_tunnel_are_never_counted() {
        for hardware_only in [true, false] {
            assert!(!is_counted(
                IF_TYPE_SOFTWARE_LOOPBACK,
                true,
                true,
                hardware_only
            ));
            assert!(!is_counted(IF_TYPE_TUNNEL, true, true, hardware_only));
        }
    }

    #[test]
    fn down_interfaces_are_never_counted() {
        assert!(!is_counted(6, false, true, false));
    }

    #[test]
    fn virtual_interfaces_count_only_without_hardware() {
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
        let (_, _, found) = totals("").expect("totals");
        assert!(!found, "고르지 않았으면 찾은 것도 없다");
    }

    fn adapter(index: u32, counted: bool, rx: u64, tx: u64) -> Adapter {
        Adapter {
            index,
            alias: format!("if{index}"),
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
    fn select_counts_only_the_chosen_alias() {
        let mut list = vec![
            adapter(1, true, 100, 10),
            adapter(2, false, 7, 3),
            adapter(3, true, 1, 2),
        ];
        // 집계에서 빠져 있던 인터페이스라도 고르면 그것만 센다.
        assert!(select(&mut list, " if2 "));
        assert_eq!(sum_counted(&list), (7, 3));
    }

    #[test]
    fn select_keeps_the_totals_when_the_alias_is_missing() {
        let mut list = vec![adapter(1, true, 100, 10), adapter(2, false, 7, 3)];
        assert!(!select(&mut list, ""));
        assert!(!select(&mut list, "gone"));
        assert_eq!(sum_counted(&list), (100, 10));
    }

    #[test]
    fn interfaces_have_names_and_no_loopback() {
        let list = interfaces();
        for entry in &list {
            assert!(!entry.alias.is_empty());
            assert!(!entry.alias.starts_with("Loopback"), "{entry:?}");
        }
        // 연결된 인터페이스가 먼저 나온다.
        let first_down = list.iter().position(|e| !e.up).unwrap_or(list.len());
        assert!(list[first_down..].iter().all(|e| !e.up));
    }

    #[test]
    fn counted_bytes_never_exceed_total_bytes() {
        let list = collect().expect("collect");
        let all_rx: u64 = list.iter().map(|a| a.rx).sum();
        let all_tx: u64 = list.iter().map(|a| a.tx).sum();
        let (rx, tx) = sum_counted(&list);
        assert!(rx <= all_rx && tx <= all_tx);
    }

    #[test]
    fn counted_flags_match_the_is_counted_rule() {
        let list = collect().expect("collect");
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
        let mut meter = NetMeter::new("");
        assert!(meter.sample().is_none());
        let rate = meter.sample().expect("두 번째 샘플");
        assert!(rate.down >= 0.0 && rate.up >= 0.0);
    }
}
