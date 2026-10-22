//! GPU 엔진 사용률 및 전용 VRAM 사용량 수집.
//!
//! 관리자 권한 없이(Non-admin) 작동한다.
//! - 어댑터 감지 및 전용 VRAM 한도: `DXGI` (`CreateDXGIFactory1`, `EnumAdapters1`)
//! - 실시간 엔진 누적 시간 및 VRAM 상주량: `gdi32.dll`의 `D3DKMTQueryStatistics`

use std::ffi::c_void;
use std::time::Instant;

use windows::Win32::Foundation::LUID;
use windows::Win32::Graphics::Dxgi::{
    CreateDXGIFactory1, DXGI_ADAPTER_FLAG_SOFTWARE, IDXGIAdapter3, IDXGIFactory4,
};
use windows::Win32::System::LibraryLoader::{GetModuleHandleW, GetProcAddress};
use windows::core::Interface;
use windows::core::PCWSTR;

use crate::logging;

const D3DKMT_QUERYSTATISTICS_ADAPTER: u32 = 0;
const D3DKMT_QUERYSTATISTICS_SEGMENT: u32 = 3;
const D3DKMT_QUERYSTATISTICS_NODE: u32 = 5;

#[repr(C)]
struct D3dKmtQueryStatistics {
    r#type: u32,
    adapter_luid: LUID,
    /// `LUID`(4바이트 정렬)와 포인터(8바이트 정렬) 사이의 패딩.
    _pad: u32,
    h_process: *mut c_void,
    /// `QueryResult` union을 바이트 배열로 받는다. `[u8; 0x308]`.
    query_result: [u8; 776],
    /// `QueryId` union의 첫 4바이트(`SegmentId`/`NodeId`).
    query_id: u32,
    /// union의 나머지. union은 4바이트지만 구조체 정렬이 8이라 4바이트가 더 붙는다.
    _query_union_pad: u32,
}

type PfnD3dKmtQueryStatistics = unsafe extern "system" fn(*mut D3dKmtQueryStatistics) -> i32;

/// 전용 VRAM 사용량. [crate::metrics::memory::Memory]와 동일한 구조다.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GpuMemory {
    pub percent: f64,
    pub used: u64,
    pub total: u64,
}

/// GPU 한 번의 샘플링 결과.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct GpuSample {
    pub percent: Option<f64>,
    pub vram: Option<GpuMemory>,
}

/// 감지된 대표 GPU 정보.
#[derive(Debug, Clone)]
pub struct GpuAdapter {
    pub luid: LUID,
    pub name: String,
    pub dedicated_vram: u64,
}

/// 누적 시간 델타를 사용률(%)로 바꾼다.
///
/// WDK 주석은 `RunningTime`을 micro-second라고 적었지만, 실측은 100ns 단위다
/// (`nvidia-smi`의 인코더 사용률과 포화 시 99%로 일치). 근거는 `docs/SPIKE.md` §14.
fn percent_from_delta(delta: u64, elapsed_100ns: f64) -> f64 {
    if elapsed_100ns <= 0.0 {
        return 0.0;
    }
    (delta as f64 / elapsed_100ns * 100.0).clamp(0.0, 100.0)
}

/// 가장 바쁜 엔진(노드)의 사용률(%).
///
/// 노드 하나가 엔진(3D·복사·인코더…) 하나에 대응한다. 아이콘은 GPU 전체 사용률을
/// 보여주므로 엔진별 사용률 중 최대값을 쓴다.
fn busiest_node_percent(prev: &[u64], now: &[u64], elapsed_100ns: f64) -> Option<f64> {
    if elapsed_100ns <= 0.0 {
        return None;
    }
    now.iter()
        .zip(prev)
        .map(|(cur, prev)| percent_from_delta(cur.saturating_sub(*prev), elapsed_100ns))
        .reduce(f64::max)
}

/// 전용 VRAM 사용률(%). 한도를 읽지 못한 어댑터는 0%로 둔다.
fn vram_percent(used: u64, total: u64) -> f64 {
    if total == 0 {
        return 0.0;
    }
    (used as f64 / total as f64 * 100.0).clamp(0.0, 100.0)
}

/// `gdi32.dll`의 `D3DKMTQueryStatistics` 함수 포인터를 얻는다.
fn query_stats_fn() -> Option<PfnD3dKmtQueryStatistics> {
    unsafe {
        let gdi32_name: Vec<u16> = "gdi32.dll\0".encode_utf16().collect();
        let module = GetModuleHandleW(PCWSTR(gdi32_name.as_ptr())).ok()?;
        let proc = GetProcAddress(module, windows::core::s!("D3DKMTQueryStatistics"))?;
        let pfn: PfnD3dKmtQueryStatistics = std::mem::transmute(proc);
        Some(pfn)
    }
}

/// 하드웨어 GPU 어댑터 목록. DXGI 열거 순서다.
pub fn adapters() -> Vec<GpuAdapter> {
    let mut list = Vec::new();
    unsafe {
        let Ok(factory) = CreateDXGIFactory1::<IDXGIFactory4>() else {
            return list;
        };

        let mut i = 0;
        while let Ok(adapter1) = factory.EnumAdapters1(i) {
            i += 1;
            let Ok(desc) = adapter1.GetDesc1() else {
                continue;
            };

            // 소프트웨어 가상 렌더러(WARP 등) 제외
            if (desc.Flags & DXGI_ADAPTER_FLAG_SOFTWARE.0 as u32) != 0 {
                continue;
            }

            // WDDM 2.0 이상을 지원하는 어댑터인지 확인
            if adapter1.cast::<IDXGIAdapter3>().is_err() {
                continue;
            }

            let len = desc
                .Description
                .iter()
                .position(|&c| c == 0)
                .unwrap_or(desc.Description.len());
            let name = String::from_utf16_lossy(&desc.Description[..len]);
            let dedicated = desc.DedicatedVideoMemory as u64;

            list.push(GpuAdapter {
                luid: desc.AdapterLuid,
                name,
                dedicated_vram: dedicated,
            });
        }
    }
    list
}

/// 이름이 같은 어댑터를 고른다. 이름이 비었거나 없으면 전용 메모리가 가장 큰 대표
/// 어댑터이고, 크기가 같으면 먼저 나온 것이다.
pub fn pick(list: Vec<GpuAdapter>, wanted: &str) -> Option<GpuAdapter> {
    let wanted = wanted.trim();
    if !wanted.is_empty() {
        if let Some(found) = list.iter().find(|adapter| adapter.name == wanted) {
            return Some(found.clone());
        }
    }
    list.into_iter().reduce(|best, next| {
        if next.dedicated_vram > best.dedicated_vram {
            next
        } else {
            best
        }
    })
}

/// 직전 샘플을 보관하며 GPU 사용률과 VRAM 사용량을 측정한다.
#[derive(Debug)]
pub struct GpuMeter {
    adapter: Option<GpuAdapter>,
    pfn_query: Option<PfnD3dKmtQueryStatistics>,
    node_count: u32,
    nb_segments: u32,
    prev_node_times: Vec<u64>,
    prev_instant: Option<Instant>,
}

impl GpuMeter {
    /// `wanted`는 어댑터 이름이다. 빈 값이거나 없는 이름이면 대표 어댑터로 물러선다.
    pub fn new(wanted: &str) -> Self {
        let adapter = pick(adapters(), wanted);
        let pfn_query = query_stats_fn();

        let mut meter = Self {
            adapter,
            pfn_query,
            node_count: 0,
            nb_segments: 0,
            prev_node_times: Vec::new(),
            prev_instant: None,
        };

        if let Some(adapter) = &meter.adapter {
            logging::info(format!(
                "GPU detected: {} (dedicated VRAM: {} MB, LUID: {:?})",
                adapter.name,
                adapter.dedicated_vram / (1024 * 1024),
                adapter.luid
            ));
            meter.init_adapter_stats();
        }

        meter.reset();
        meter
    }

    pub fn adapter(&self) -> Option<&GpuAdapter> {
        self.adapter.as_ref()
    }

    /// 다른 어댑터로 바꾼다. 노드·세그먼트 수가 어댑터마다 달라 처음부터 다시 잡는다.
    pub fn set_adapter(&mut self, wanted: &str) {
        *self = Self::new(wanted);
    }

    /// 어댑터의 노드 수와 세그먼트 수를 초기 조회한다.
    fn init_adapter_stats(&mut self) {
        let (Some(adapter), Some(pfn)) = (&self.adapter, self.pfn_query) else {
            return;
        };

        let mut stats: D3dKmtQueryStatistics = unsafe { std::mem::zeroed() };
        stats.r#type = D3DKMT_QUERYSTATISTICS_ADAPTER;
        stats.adapter_luid = adapter.luid;

        let status = unsafe { pfn(&mut stats) };
        if status == 0 {
            // D3DKMT_QUERYSTATISTICS_ADAPTER_INFORMATION: NbSegments(0), NodeCount(4).
            self.nb_segments = u32::from_ne_bytes(stats.query_result[0..4].try_into().unwrap());
            self.node_count = u32::from_ne_bytes(stats.query_result[4..8].try_into().unwrap());
            self.prev_node_times = vec![0; self.node_count as usize];
        }
    }

    /// 현재 각 노드의 누적 RunningTime(100ns)을 읽어온다.
    fn read_node_times(&self, out: &mut [u64]) {
        let (Some(adapter), Some(pfn)) = (&self.adapter, self.pfn_query) else {
            return;
        };

        for (node_id, slot) in out.iter_mut().enumerate() {
            let mut stats: D3dKmtQueryStatistics = unsafe { std::mem::zeroed() };
            stats.r#type = D3DKMT_QUERYSTATISTICS_NODE;
            stats.adapter_luid = adapter.luid;
            stats.query_id = node_id as u32;

            if unsafe { pfn(&mut stats) } == 0 {
                // NODE_INFORMATION.GlobalInformation.RunningTime(0, 100ns 단위).
                *slot = u64::from_ne_bytes(stats.query_result[0..8].try_into().unwrap());
            }
        }
    }

    /// 실시간 전용 VRAM 상주량(bytes)을 읽어온다.
    fn read_vram_resident(&self) -> Option<u64> {
        let (Some(adapter), Some(pfn)) = (&self.adapter, self.pfn_query) else {
            return None;
        };

        let mut total_resident = 0u64;

        for seg_id in 0..self.nb_segments {
            let mut stats: D3dKmtQueryStatistics = unsafe { std::mem::zeroed() };
            stats.r#type = D3DKMT_QUERYSTATISTICS_SEGMENT;
            stats.adapter_luid = adapter.luid;
            stats.query_id = seg_id;

            if unsafe { pfn(&mut stats) } == 0 {
                // SEGMENT_INFORMATION: CommitLimit(0), BytesCommitted(8), BytesResident(16).
                let resident = u64::from_ne_bytes(stats.query_result[16..24].try_into().unwrap());
                // 같은 구조체의 Aperture(40). 부울로 "이 세그먼트가 aperture인가"를 뜻한다.
                let aperture = u32::from_ne_bytes(stats.query_result[40..44].try_into().unwrap());

                // Aperture == 0 인 세그먼트가 시스템 공유가 아닌 로컬 전용 VRAM
                if aperture == 0 {
                    total_resident = total_resident.saturating_add(resident);
                }
            }
        }

        Some(total_resident)
    }

    /// 이전 표본 이후의 사용률을 계산하고 새 기준점을 잡는다.
    pub fn sample(&mut self) -> GpuSample {
        let Some(adapter) = &self.adapter else {
            return GpuSample::default();
        };

        let now = Instant::now();
        let prev_instant = self.prev_instant.replace(now);

        // 1. GPU 엔진 점유율 (%)
        let mut cur_node_times = vec![0u64; self.node_count as usize];
        self.read_node_times(&mut cur_node_times);

        let gpu_percent = prev_instant.and_then(|prev_time| {
            let elapsed_nanos = now.duration_since(prev_time).as_nanos();
            busiest_node_percent(
                &self.prev_node_times,
                &cur_node_times,
                elapsed_nanos as f64 / 100.0,
            )
        });

        self.prev_node_times = cur_node_times;

        // 2. VRAM 사용량
        let vram = self.read_vram_resident().map(|used| {
            let total = adapter.dedicated_vram;
            GpuMemory {
                percent: vram_percent(used, total),
                used,
                total,
            }
        });

        GpuSample {
            percent: gpu_percent,
            vram,
        }
    }

    /// 절전 복귀나 긴 공백 뒤에 기준점을 재설정한다.
    pub fn reset(&mut self) {
        if self.node_count > 0 {
            let mut times = vec![0u64; self.node_count as usize];
            self.read_node_times(&mut times);
            self.prev_node_times = times;
        }
        self.prev_instant = Some(Instant::now());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// WDK 주석은 micro-second라고 적었지만 실측은 100ns다. 단위 가정을 여기에 고정한다.
    #[test]
    fn percent_uses_100ns_ticks() {
        // 노드가 1초(10^7 tick) 내내 돌면 100%다.
        assert_eq!(percent_from_delta(10_000_000, 10_000_000.0), 100.0);
        assert_eq!(percent_from_delta(2_500_000, 10_000_000.0), 25.0);
        // 표본 간격보다 긴 델타는 100%로 자른다.
        assert_eq!(percent_from_delta(30_000_000, 10_000_000.0), 100.0);
        assert_eq!(percent_from_delta(0, 10_000_000.0), 0.0);
        assert_eq!(percent_from_delta(1_000, 0.0), 0.0);
    }

    #[test]
    fn the_busiest_node_wins() {
        let prev = [1_000_000, 5_000_000, 9_000_000];
        let now = [1_500_000, 7_500_000, 9_000_000];
        // 노드0은 5%, 노드1은 25%, 노드2는 0%를 돌았다.
        let pct = busiest_node_percent(&prev, &now, 10_000_000.0).expect("사용률");
        assert!(
            (pct - 25.0).abs() < 1e-9,
            "가장 바쁜 노드의 값이 나와야 한다: {pct}"
        );
    }

    /// 시간이 멈췄거나(절전) 표본이 겹치면 값을 만들지 않는다.
    #[test]
    fn a_missing_baseline_has_no_reading() {
        assert_eq!(busiest_node_percent(&[], &[], 10_000_000.0), None);
        assert_eq!(busiest_node_percent(&[1, 2], &[3, 4], 0.0), None);
    }

    #[test]
    fn vram_percent_handles_a_zero_limit() {
        assert_eq!(vram_percent(3, 8), 37.5);
        assert_eq!(vram_percent(9, 8), 100.0);
        assert_eq!(vram_percent(5, 0), 0.0);
    }

    /// 손으로 적은 배치가 x64 ABI와 같은지 고정한다. WDK도 같은 크기를 강제한다
    /// (`C_ASSERT(sizeof(D3DKMT_QUERYSTATISTICS) == 0x328)`).
    #[test]
    #[cfg(target_pointer_width = "64")]
    fn kmt_statistics_layout_matches_the_wdk() {
        use std::mem::{offset_of, size_of};
        assert_eq!(size_of::<D3dKmtQueryStatistics>(), 0x328);
        assert_eq!(offset_of!(D3dKmtQueryStatistics, adapter_luid), 4);
        assert_eq!(offset_of!(D3dKmtQueryStatistics, h_process), 16);
        assert_eq!(offset_of!(D3dKmtQueryStatistics, query_result), 24);
        assert_eq!(offset_of!(D3dKmtQueryStatistics, query_id), 800);
    }

    fn adapter(name: &str, vram: u64) -> GpuAdapter {
        GpuAdapter {
            luid: LUID::default(),
            name: name.to_string(),
            dedicated_vram: vram,
        }
    }

    #[test]
    fn pick_prefers_the_named_adapter() {
        let list = || {
            vec![
                adapter("iGPU", 128),
                adapter("dGPU", 8192),
                adapter("eGPU", 8192),
            ]
        };
        assert_eq!(pick(list(), "iGPU").unwrap().name, "iGPU");
        assert_eq!(pick(list(), " iGPU ").unwrap().name, "iGPU");
        // 이름이 비었거나 사라졌으면 가장 큰 전용 메모리, 같으면 먼저 나온 것.
        assert_eq!(pick(list(), "").unwrap().name, "dGPU");
        assert_eq!(pick(list(), "gone").unwrap().name, "dGPU");
        assert!(pick(Vec::new(), "").is_none());
    }

    #[test]
    fn adapter_detection_reports_a_name() {
        if let Some(adapter) = pick(adapters(), "") {
            assert!(!adapter.name.is_empty());
        }
    }

    /// 실제 드라이버를 호출하는 스모크 테스트. GPU가 없으면 검사할 값도 없다.
    #[test]
    fn meter_reports_values_in_range() {
        let mut meter = GpuMeter::new("");
        std::thread::sleep(std::time::Duration::from_millis(100));
        let sample = meter.sample();
        if meter.adapter().is_some() {
            if let Some(percent) = sample.percent {
                assert!((0.0..=100.0).contains(&percent), "{percent}");
            }
            if let Some(vram) = sample.vram {
                assert!((0.0..=100.0).contains(&vram.percent));
                // 전용 메모리가 없는 어댑터(CI 러너의 WARP 등)는 한도가 0이고
                // 사용률은 0%로 둔다.
                if vram.total == 0 {
                    assert_eq!(vram.percent, 0.0);
                }
            }
        }
    }
}
