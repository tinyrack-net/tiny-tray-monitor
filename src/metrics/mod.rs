//! 지표 수집. 관리자 권한이 필요 없는 Win32 API만 쓴다.
//!
//! - CPU: `GetSystemTimes` 누적 시간 델타
//! - 메모리: `GlobalMemoryStatusEx`
//! - 네트워크: `GetIfTable2` 인터페이스별 누적 바이트 델타
//! - GPU: `D3DKMTQueryStatistics` 노드 실행 시간 델타 및 전용 세그먼트 상주량
//! - 디스크: `GetDiskFreeSpaceExW` 사용량, `IOCTL_DISK_PERFORMANCE` 유휴 시간 델타

pub mod cpu;
pub mod disk;
pub mod format;
pub mod gpu;
pub mod memory;
pub mod network;

use crate::config::Devices;

/// 한 번의 샘플링 결과. 첫 샘플에서는 비어 있을 수 있다.
#[derive(Debug, Clone, Copy, Default)]
pub struct Sample {
    pub cpu_percent: Option<f64>,
    pub memory: Option<memory::Memory>,
    pub net: Option<network::Rate>,
    pub gpu_percent: Option<f64>,
    pub gpu_vram: Option<gpu::GpuMemory>,
    pub disk_usage: Option<disk::Usage>,
    pub disk_active: Option<disk::Activity>,
}

/// 모든 지표를 함께 샘플링한다.
#[derive(Debug)]
pub struct Sampler {
    cpu: cpu::CpuMeter,
    net: network::NetMeter,
    gpu: gpu::GpuMeter,
    disk: disk::DiskMeter,
}

impl Sampler {
    /// 기준점을 잡기 위해 한 번 샘플링하고 버린다.
    pub fn new(exclude_virtual_adapters: bool, devices: &Devices) -> Self {
        let mut sampler = Self {
            cpu: cpu::CpuMeter::new(),
            net: network::NetMeter::new(exclude_virtual_adapters, &devices.network),
            gpu: gpu::GpuMeter::new(&devices.gpu),
            disk: disk::DiskMeter::new(&devices.disk),
        };
        sampler.prime();
        sampler
    }

    /// 델타 기준점만 다시 잡는다. 절전 복귀나 폴링 주기 변경 후에 쓴다.
    pub fn prime(&mut self) {
        self.cpu.reset();
        self.net.reset();
        self.gpu.reset();
        self.disk.reset();
        let _ = self.net.sample();
        let _ = self.gpu.sample();
    }

    pub fn sample(&mut self) -> Sample {
        let gpu_sample = self.gpu.sample();
        let disk_sample = self.disk.sample();
        Sample {
            cpu_percent: self.cpu.sample(),
            memory: memory::read(),
            net: self.net.sample(),
            gpu_percent: gpu_sample.percent,
            gpu_vram: gpu_sample.vram,
            disk_usage: disk_sample.usage,
            disk_active: disk_sample.activity,
        }
    }

    pub fn set_exclude_virtual_adapters(&mut self, exclude: bool) {
        self.net.set_exclude_virtual(exclude);
    }

    pub fn gpu_adapter(&self) -> Option<&gpu::GpuAdapter> {
        self.gpu.adapter()
    }

    /// 네트워크 인터페이스를 바꾼다. 빈 값이면 집계 대상 전부의 합.
    pub fn set_network_device(&mut self, device: &str) {
        self.net.set_device(device);
    }

    /// 지금 재는 네트워크 인터페이스. 전부의 합이면 `None`.
    pub fn network_device(&self) -> Option<&str> {
        self.net.device()
    }

    /// GPU 어댑터를 바꾼다. 빈 값이면 전용 메모리가 가장 큰 어댑터.
    pub fn set_gpu_device(&mut self, device: &str) {
        self.gpu.set_adapter(device);
    }

    /// 드라이브를 바꾼다. 빈 값이면 시스템 드라이브.
    pub fn set_disk_device(&mut self, device: &str) {
        self.disk.set_drive(device);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sampler_returns_real_values() {
        let mut sampler = Sampler::new(true, &Devices::default());
        std::thread::sleep(std::time::Duration::from_millis(120));
        let sample = sampler.sample();

        let cpu = sample.cpu_percent.expect("두 번째 샘플부터 값이 나온다");
        assert!((0.0..=100.0).contains(&cpu), "cpu={cpu}");

        let memory = sample.memory.expect("메모리");
        assert!((0.0..=100.0).contains(&memory.percent));

        let net = sample.net.expect("네트워크");
        assert!(net.down >= 0.0 && net.up >= 0.0);

        let usage = sample.disk_usage.expect("시스템 드라이브");
        assert!((0.0..=100.0).contains(&usage.percent));

        if sampler.gpu_adapter().is_some() {
            if let Some(gpu) = sample.gpu_percent {
                assert!((0.0..=100.0).contains(&gpu));
            }
            if let Some(vram) = sample.gpu_vram {
                assert!((0.0..=100.0).contains(&vram.percent));
            }
        }
    }

    /// 절전 복귀를 흉내내면 값이 튀지 않고 다시 기준점을 잡아야 한다.
    #[test]
    fn prime_resets_the_baseline() {
        let mut sampler = Sampler::new(true, &Devices::default());
        std::thread::sleep(std::time::Duration::from_millis(120));
        let first = sampler.sample();
        assert!(first.net.is_some());

        sampler.prime();
        let after = sampler.sample();
        assert!(after.net.is_some(), "prime 직후에도 샘플이 나온다");
        let net = after.net.unwrap();
        assert!(
            net.down < 1024.0 * 1024.0 * 100.0,
            "튀는 값이 나오면 안 된다"
        );
    }
}
