//! 디스크 사용량과 로드율(활성 시간).
//!
//! 관리자 권한 없이 작동한다(`docs/SPIKE.md` §17).
//! - 사용량: `GetDiskFreeSpaceExW`
//! - 로드율: 볼륨이 놓인 물리 디스크(`IOCTL_VOLUME_GET_VOLUME_DISK_EXTENTS`)를 접근 권한 0으로
//!   열고 `IOCTL_DISK_PERFORMANCE`의 `IdleTime`/`QueryTime` 델타로 계산한다. 작업 관리자의
//!   "활성 시간"과 같은 공식이다.
//!
//! 핸들은 표본마다 열고 닫는다. 열어 두면 USB 디스크의 안전한 제거를 막을 수 있다.

use core::ffi::c_void;
use core::mem::size_of;

use windows::Win32::Foundation::{CloseHandle, HANDLE};
use windows::Win32::Storage::FileSystem::{
    CreateFileW, FILE_FLAGS_AND_ATTRIBUTES, FILE_SHARE_READ, FILE_SHARE_WRITE, GetDiskFreeSpaceExW,
    GetDriveTypeW, GetLogicalDrives, GetVolumeInformationW, IOCTL_VOLUME_GET_VOLUME_DISK_EXTENTS,
    OPEN_EXISTING,
};
use windows::Win32::System::IO::DeviceIoControl;
use windows::Win32::System::Ioctl::{
    DISK_PERFORMANCE, IOCTL_DISK_PERFORMANCE, IOCTL_STORAGE_QUERY_PROPERTY, PropertyStandardQuery,
    STORAGE_PROPERTY_QUERY, StorageDeviceProperty,
};
use windows::Win32::System::SystemInformation::GetSystemWindowsDirectoryW;
use windows::core::PCWSTR;

use crate::logging;

/// `GetDriveTypeW`의 `DRIVE_REMOVABLE`
const DRIVE_REMOVABLE: u32 = 2;
/// `GetDriveTypeW`의 `DRIVE_FIXED`
const DRIVE_FIXED: u32 = 3;

/// 드라이브 하나의 공간 사용량.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Usage {
    pub letter: char,
    pub percent: f64,
    pub used: u64,
    pub total: u64,
}

/// 드라이브가 놓인 물리 디스크의 로드율과 처리량.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Activity {
    pub letter: char,
    /// 활성 시간(%). 디스크가 여럿이면 가장 바쁜 디스크의 값이다.
    pub percent: f64,
    /// 초당 읽은 바이트. 디스크가 여럿이면 합이다.
    pub read: f64,
    /// 초당 쓴 바이트.
    pub write: f64,
}

/// 디스크 한 번의 샘플링 결과.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct DiskSample {
    pub usage: Option<Usage>,
    pub activity: Option<Activity>,
}

/// 설정 창의 드라이브 목록에 쓰는 정보.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Drive {
    pub letter: char,
    pub label: String,
    pub removable: bool,
    pub system: bool,
    pub total: u64,
    pub free: u64,
}

/// `IOCTL_DISK_PERFORMANCE`의 누적 카운터. 시간은 100ns 단위다.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Counters {
    pub idle: i64,
    pub query: i64,
    pub read: i64,
    pub written: i64,
}

/// 설정 값(`"D:"`, `"d"`, `"D:\\"`)을 드라이브 문자로. 빈 값이나 잘못된 값은 `None`(자동).
pub fn parse_letter(value: &str) -> Option<char> {
    let first = value.trim().chars().next()?.to_ascii_uppercase();
    first.is_ascii_uppercase().then_some(first)
}

/// 드라이브 문자를 설정 값으로. (`'D'` -> `"D:"`)
pub fn letter_value(letter: char) -> String {
    format!("{}:", letter.to_ascii_uppercase())
}

/// 사용률(%). 전체가 0이면 0%다.
pub fn used_percent(total: u64, free: u64) -> f64 {
    if total == 0 {
        return 0.0;
    }
    let used = total.saturating_sub(free.min(total));
    (used as f64 / total as f64 * 100.0).clamp(0.0, 100.0)
}

/// 두 카운터 사이의 (활성 시간 %, 읽기 B/s, 쓰기 B/s).
///
/// 경과 시간이 0이거나 카운터가 역행하면 `None`이다.
pub fn activity_between(prev: &Counters, cur: &Counters) -> Option<(f64, f64, f64)> {
    let elapsed = cur.query.checked_sub(prev.query)?;
    if elapsed <= 0 {
        return None;
    }
    let idle = cur.idle.saturating_sub(prev.idle).clamp(0, elapsed);
    let percent = ((1.0 - idle as f64 / elapsed as f64) * 100.0).clamp(0.0, 100.0);
    let seconds = elapsed as f64 / 10_000_000.0;
    let rate = |cur: i64, prev: i64| cur.saturating_sub(prev).max(0) as f64 / seconds;
    Some((
        percent,
        rate(cur.read, prev.read),
        rate(cur.written, prev.written),
    ))
}

/// `VOLUME_DISK_EXTENTS` 바이트에서 물리 디스크 번호를 뽑는다. 중복은 한 번만 넣는다.
///
/// 배치: `NumberOfDiskExtents`(u32, 0) + 패딩(4) + `DISK_EXTENT`(24바이트, `DiskNumber`가 첫
/// u32) 배열.
pub fn disk_numbers(bytes: &[u8]) -> Vec<u32> {
    const HEADER: usize = 8;
    const EXTENT: usize = 24;
    let Some(count) = bytes.get(0..4) else {
        return Vec::new();
    };
    let count = u32::from_ne_bytes(count.try_into().unwrap_or_default()) as usize;
    let mut numbers = Vec::new();
    for index in 0..count {
        let start = HEADER + index * EXTENT;
        let Some(field) = bytes.get(start..start + 4) else {
            break;
        };
        let number = u32::from_ne_bytes(field.try_into().unwrap_or_default());
        if !numbers.contains(&number) {
            numbers.push(number);
        }
    }
    numbers
}

/// `STORAGE_DEVICE_DESCRIPTOR` 바이트에서 제조사와 제품 이름을 읽는다.
///
/// `VendorIdOffset`(12), `ProductIdOffset`(16)이 가리키는 NUL 종료 ASCII 문자열이다. 0이면 없다.
pub fn descriptor_name(bytes: &[u8]) -> Option<String> {
    let text_at = |field: usize| -> String {
        let offset = bytes
            .get(field..field + 4)
            .map(|raw| u32::from_ne_bytes(raw.try_into().unwrap_or_default()) as usize)
            .unwrap_or(0);
        if offset == 0 || offset >= bytes.len() {
            return String::new();
        }
        let tail = &bytes[offset..];
        let end = tail.iter().position(|&b| b == 0).unwrap_or(tail.len());
        String::from_utf8_lossy(&tail[..end]).trim().to_string()
    };
    let vendor = text_at(12);
    let product = text_at(16);
    match (vendor.is_empty(), product.is_empty()) {
        (true, true) => None,
        (true, false) => Some(product),
        (false, true) => Some(vendor),
        (false, false) => Some(format!("{vendor} {product}")),
    }
}

/// 표본마다 열고 닫는 장치 핸들.
struct Device(HANDLE);

impl Drop for Device {
    fn drop(&mut self) {
        let _ = unsafe { CloseHandle(self.0) };
    }
}

/// 장치를 접근 권한 0으로 연다. 이 권한이면 관리자가 아니어도 IOCTL 질의가 된다.
fn open(path: &str) -> Option<Device> {
    let wide = crate::win32::wide(path);
    let handle = unsafe {
        CreateFileW(
            PCWSTR(wide.as_ptr()),
            0,
            FILE_SHARE_READ | FILE_SHARE_WRITE,
            None,
            OPEN_EXISTING,
            FILE_FLAGS_AND_ATTRIBUTES(0),
            None,
        )
    }
    .ok()?;
    Some(Device(handle))
}

/// IOCTL을 보내고 돌려받은 바이트 수를 준다.
fn ioctl(device: &Device, code: u32, input: Option<&[u8]>, output: &mut [u8]) -> Option<usize> {
    let mut returned = 0u32;
    unsafe {
        DeviceIoControl(
            device.0,
            code,
            input.map(|bytes| bytes.as_ptr() as *const c_void),
            input.map_or(0, |bytes| bytes.len() as u32),
            Some(output.as_mut_ptr() as *mut c_void),
            output.len() as u32,
            Some(&mut returned),
            None,
        )
    }
    .ok()?;
    Some(returned as usize)
}

/// 8바이트 정렬 버퍼. IOCTL 출력 구조체에는 i64 필드가 있다.
fn aligned_buffer(bytes: usize) -> Vec<u64> {
    vec![0u64; bytes.div_ceil(8)]
}

fn as_bytes_mut(buffer: &mut [u64]) -> &mut [u8] {
    unsafe { std::slice::from_raw_parts_mut(buffer.as_mut_ptr() as *mut u8, buffer.len() * 8) }
}

/// 물리 디스크의 누적 카운터.
pub fn read_counters(disk: u32) -> Option<Counters> {
    let device = open(&format!(r"\\.\PhysicalDrive{disk}"))?;
    let mut buffer = aligned_buffer(size_of::<DISK_PERFORMANCE>());
    ioctl(
        &device,
        IOCTL_DISK_PERFORMANCE,
        None,
        as_bytes_mut(&mut buffer),
    )?;
    let perf: DISK_PERFORMANCE =
        unsafe { std::ptr::read_unaligned(buffer.as_ptr() as *const DISK_PERFORMANCE) };
    Some(Counters {
        idle: perf.IdleTime,
        query: perf.QueryTime,
        read: perf.BytesRead,
        written: perf.BytesWritten,
    })
}

/// 드라이브가 놓인 물리 디스크 번호. 스팬·RAID 볼륨이면 여럿이다.
pub fn disks_of(letter: char) -> Vec<u32> {
    let Some(device) = open(&format!(r"\\.\{}:", letter.to_ascii_uppercase())) else {
        return Vec::new();
    };
    let mut buffer = aligned_buffer(512);
    let bytes = as_bytes_mut(&mut buffer);
    match ioctl(&device, IOCTL_VOLUME_GET_VOLUME_DISK_EXTENTS, None, bytes) {
        Some(_) => disk_numbers(bytes),
        None => Vec::new(),
    }
}

/// 물리 디스크의 모델명. 설정 창의 정보 줄에 쓴다.
pub fn model(disk: u32) -> Option<String> {
    let device = open(&format!(r"\\.\PhysicalDrive{disk}"))?;
    let query = STORAGE_PROPERTY_QUERY {
        PropertyId: StorageDeviceProperty,
        QueryType: PropertyStandardQuery,
        AdditionalParameters: [0],
    };
    let input = unsafe {
        std::slice::from_raw_parts(
            &query as *const STORAGE_PROPERTY_QUERY as *const u8,
            size_of::<STORAGE_PROPERTY_QUERY>(),
        )
    };
    let mut buffer = aligned_buffer(1024);
    let bytes = as_bytes_mut(&mut buffer);
    let returned = ioctl(&device, IOCTL_STORAGE_QUERY_PROPERTY, Some(input), bytes)?;
    descriptor_name(&bytes[..returned.min(bytes.len())])
}

/// Windows가 설치된 드라이브. 읽지 못하면 `C`.
pub fn system_letter() -> char {
    let mut buffer = [0u16; 260];
    let length = unsafe { GetSystemWindowsDirectoryW(Some(&mut buffer)) } as usize;
    String::from_utf16_lossy(&buffer[..length.min(buffer.len())])
        .chars()
        .next()
        .and_then(|c| parse_letter(&c.to_string()))
        .unwrap_or('C')
}

/// 드라이브의 (전체, 여유) 바이트. 미디어가 없으면 `None`.
fn space(letter: char) -> Option<(u64, u64)> {
    let root = crate::win32::wide(&format!("{}:\\", letter.to_ascii_uppercase()));
    let (mut total, mut free) = (0u64, 0u64);
    unsafe {
        GetDiskFreeSpaceExW(
            PCWSTR(root.as_ptr()),
            None,
            Some(&mut total),
            Some(&mut free),
        )
    }
    .ok()?;
    Some((total, free))
}

/// 드라이브 사용량.
pub fn usage(letter: char) -> Option<Usage> {
    let (total, free) = space(letter)?;
    Some(Usage {
        letter,
        percent: used_percent(total, free),
        used: total.saturating_sub(free.min(total)),
        total,
    })
}

fn volume_label(letter: char) -> String {
    let root = crate::win32::wide(&format!("{letter}:\\"));
    let mut name = [0u16; 261];
    let ok = unsafe {
        GetVolumeInformationW(
            PCWSTR(root.as_ptr()),
            Some(&mut name),
            None,
            None,
            None,
            None,
        )
    }
    .is_ok();
    if !ok {
        return String::new();
    }
    let end = name.iter().position(|&c| c == 0).unwrap_or(name.len());
    String::from_utf16_lossy(&name[..end]).trim().to_string()
}

/// 고정·이동식 드라이브 중 미디어가 있는 것. 문자 순서다.
pub fn drives() -> Vec<Drive> {
    let mask = unsafe { GetLogicalDrives() };
    let system = system_letter();
    let mut list = Vec::new();
    for (bit, letter) in ('A'..='Z').enumerate() {
        if mask & (1 << bit) == 0 {
            continue;
        }
        let root = crate::win32::wide(&format!("{letter}:\\"));
        let kind = unsafe { GetDriveTypeW(PCWSTR(root.as_ptr())) };
        if kind != DRIVE_FIXED && kind != DRIVE_REMOVABLE {
            continue;
        }
        let Some((total, free)) = space(letter) else {
            continue;
        };
        list.push(Drive {
            letter,
            label: volume_label(letter),
            removable: kind == DRIVE_REMOVABLE,
            system: letter == system,
            total,
            free,
        });
    }
    list
}

/// 고른 드라이브. 없으면(빼낸 USB 등) 시스템 드라이브로 물러선다.
pub fn resolve(wanted: Option<char>) -> char {
    match wanted {
        Some(letter) if space(letter).is_some() => letter,
        _ => system_letter(),
    }
}

/// 직전 카운터를 보관하며 드라이브의 사용량과 로드율을 잰다.
#[derive(Debug)]
pub struct DiskMeter {
    wanted: Option<char>,
    letter: char,
    disks: Vec<u32>,
    prev: Vec<(u32, Counters)>,
}

impl DiskMeter {
    /// `wanted`는 설정 값(`"D:"`). 빈 값이면 시스템 드라이브다.
    pub fn new(wanted: &str) -> Self {
        let mut meter = Self {
            wanted: parse_letter(wanted),
            letter: system_letter(),
            disks: Vec::new(),
            prev: Vec::new(),
        };
        meter.reset();
        meter
    }

    /// 드라이브를 바꾸고 기준점을 다시 잡는다.
    pub fn set_drive(&mut self, wanted: &str) {
        self.wanted = parse_letter(wanted);
        self.reset();
    }

    /// 드라이브와 물리 디스크를 다시 찾고 기준점을 잡는다. 절전 복귀 뒤에도 부른다.
    pub fn reset(&mut self) {
        let letter = resolve(self.wanted);
        let disks = disks_of(letter);
        // 디스크를 못 찾으면 표본마다 다시 부르므로, 바뀐 때만 남긴다.
        if letter != self.letter || disks != self.disks {
            logging::info(format!("disk readouts follow {letter}: (disks {disks:?})"));
            if let Some(wanted) = self.wanted.filter(|wanted| *wanted != letter) {
                logging::warn(format!(
                    "drive {wanted}: is not available, using the system drive"
                ));
            }
        }
        self.letter = letter;
        self.disks = disks;
        self.prev = self.read_all();
    }

    fn read_all(&self) -> Vec<(u32, Counters)> {
        self.disks
            .iter()
            .filter_map(|disk| read_counters(*disk).map(|counters| (*disk, counters)))
            .collect()
    }

    pub fn sample(&mut self) -> DiskSample {
        let space = usage(self.letter);
        // 드라이브가 사라졌거나(USB 제거) 디스크를 아직 못 찾았으면 다시 찾는다. 로드율은
        // 새 기준점에서 다음 표본부터 나온다.
        if space.is_none() || self.disks.is_empty() {
            self.reset();
            return DiskSample {
                usage: space.or_else(|| usage(self.letter)),
                activity: None,
            };
        }
        let usage = space;

        let current = self.read_all();
        let mut activity: Option<Activity> = None;
        for (disk, counters) in &current {
            let Some((_, prev)) = self.prev.iter().find(|(number, _)| number == disk) else {
                continue;
            };
            let Some((percent, read, write)) = activity_between(prev, counters) else {
                continue;
            };
            let entry = activity.get_or_insert(Activity {
                letter: self.letter,
                percent: 0.0,
                read: 0.0,
                write: 0.0,
            });
            entry.percent = entry.percent.max(percent);
            entry.read += read;
            entry.write += write;
        }
        self.prev = current;

        DiskSample { usage, activity }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn counters(idle: i64, query: i64, read: i64, written: i64) -> Counters {
        Counters {
            idle,
            query,
            read,
            written,
        }
    }

    #[test]
    fn letters_are_normalised() {
        assert_eq!(parse_letter("D:"), Some('D'));
        assert_eq!(parse_letter("d"), Some('D'));
        assert_eq!(parse_letter(r"e:\"), Some('E'));
        assert_eq!(parse_letter(""), None, "빈 값은 자동(시스템 드라이브)");
        assert_eq!(parse_letter("  "), None);
        assert_eq!(parse_letter("1:"), None);
        assert_eq!(letter_value('d'), "D:");
    }

    #[test]
    fn used_percent_handles_edges() {
        assert_eq!(used_percent(100, 25), 75.0);
        assert_eq!(used_percent(100, 100), 0.0);
        assert_eq!(used_percent(100, 0), 100.0);
        assert_eq!(used_percent(0, 0), 0.0);
        assert_eq!(used_percent(100, 500), 0.0, "여유가 전체보다 크면 0%");
    }

    /// 1초(10^7 tick) 중 0.25초가 유휴면 75% 활성이다.
    #[test]
    fn activity_is_one_minus_idle() {
        let prev = counters(0, 0, 0, 0);
        let cur = counters(2_500_000, 10_000_000, 4096, 8192);
        let (percent, read, write) = activity_between(&prev, &cur).expect("값");
        assert!((percent - 75.0).abs() < 1e-9, "{percent}");
        assert_eq!(read, 4096.0);
        assert_eq!(write, 8192.0);
    }

    #[test]
    fn activity_is_clamped() {
        let prev = counters(0, 0, 0, 0);
        // 유휴가 경과보다 길게 잡혀도 0% 아래로 내려가지 않는다.
        let (percent, _, _) = activity_between(&prev, &counters(20, 10, 0, 0)).unwrap();
        assert_eq!(percent, 0.0);
        // 유휴가 역행하면 100%로 자른다.
        let (percent, _, _) =
            activity_between(&counters(50, 0, 0, 0), &counters(0, 10, 0, 0)).unwrap();
        assert_eq!(percent, 100.0);
    }

    #[test]
    fn activity_needs_time_to_pass() {
        let same = counters(1, 10, 2, 3);
        assert_eq!(activity_between(&same, &same), None);
        assert_eq!(
            activity_between(&counters(0, 10, 0, 0), &counters(0, 5, 0, 0)),
            None,
            "QueryTime이 역행하면 값을 만들지 않는다"
        );
    }

    #[test]
    fn byte_counters_going_backwards_read_as_zero() {
        let (_, read, write) =
            activity_between(&counters(0, 0, 100, 100), &counters(0, 10_000_000, 0, 50)).unwrap();
        assert_eq!(read, 0.0);
        assert_eq!(write, 0.0);
    }

    #[test]
    fn extents_are_parsed_and_deduplicated() {
        let mut bytes = vec![0u8; 8 + 24 * 3];
        bytes[0..4].copy_from_slice(&3u32.to_ne_bytes());
        bytes[8..12].copy_from_slice(&1u32.to_ne_bytes());
        bytes[32..36].copy_from_slice(&4u32.to_ne_bytes());
        bytes[56..60].copy_from_slice(&1u32.to_ne_bytes());
        assert_eq!(disk_numbers(&bytes), vec![1, 4]);

        // 개수가 버퍼보다 크면 있는 만큼만 읽는다.
        bytes[0..4].copy_from_slice(&9u32.to_ne_bytes());
        assert_eq!(disk_numbers(&bytes), vec![1, 4]);
        assert!(disk_numbers(&[]).is_empty());
    }

    #[test]
    fn descriptor_names_join_vendor_and_product() {
        let mut bytes = vec![0u8; 64];
        bytes[12..16].copy_from_slice(&40u32.to_ne_bytes());
        bytes[16..20].copy_from_slice(&48u32.to_ne_bytes());
        bytes[40..44].copy_from_slice(b"USB ");
        bytes[48..58].copy_from_slice(b"Stick 3.0 ");
        assert_eq!(descriptor_name(&bytes).as_deref(), Some("USB Stick 3.0"));

        // NVMe는 제조사 칸이 비어 있다(실측).
        bytes[12..16].copy_from_slice(&0u32.to_ne_bytes());
        assert_eq!(descriptor_name(&bytes).as_deref(), Some("Stick 3.0"));

        bytes[16..20].copy_from_slice(&0u32.to_ne_bytes());
        assert_eq!(descriptor_name(&bytes), None);
    }

    #[test]
    fn the_system_drive_is_listed() {
        let system = system_letter();
        let list = drives();
        let entry = list
            .iter()
            .find(|drive| drive.letter == system)
            .expect("시스템 드라이브");
        assert!(entry.system);
        assert!(entry.total > 0 && entry.free <= entry.total);
    }

    #[test]
    fn a_missing_drive_falls_back_to_the_system_drive() {
        // 이 기기에 없을 법한 드라이브 문자를 고른다.
        let missing = ('A'..='Z')
            .rev()
            .find(|letter| space(*letter).is_none())
            .expect("빈 드라이브 문자");
        assert_eq!(resolve(Some(missing)), system_letter());
        assert_eq!(resolve(None), system_letter());
    }

    /// 실제 드라이버를 부르는 스모크 테스트.
    #[test]
    fn meter_reports_values_in_range() {
        let mut meter = DiskMeter::new("");
        assert_eq!(meter.letter, system_letter());
        std::thread::sleep(std::time::Duration::from_millis(100));
        let sample = meter.sample();
        let usage = sample.usage.expect("시스템 드라이브 사용량");
        assert!((0.0..=100.0).contains(&usage.percent));
        assert!(usage.used <= usage.total);
        if let Some(activity) = sample.activity {
            assert!((0.0..=100.0).contains(&activity.percent));
            assert!(activity.read >= 0.0 && activity.write >= 0.0);
        }
    }
}
