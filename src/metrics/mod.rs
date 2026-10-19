//! 지표 수집. 관리자 권한이 필요 없는 Win32 API만 쓴다.
//!
//! - CPU: `GetSystemTimes` 누적 시간 델타
//! - 메모리: `GlobalMemoryStatusEx`
//! - 네트워크: `GetIfTable2` 인터페이스별 누적 바이트 델타

pub mod cpu;
pub mod format;
pub mod memory;
pub mod network;

/// 한 번의 샘플링 결과. 첫 샘플에서는 비어 있을 수 있다.
#[derive(Debug, Clone, Copy, Default)]
pub struct Sample {
    pub cpu_percent: Option<f64>,
    pub memory: Option<memory::Memory>,
    pub net: Option<network::Rate>,
}

/// 세 지표를 함께 샘플링한다.
#[derive(Debug)]
pub struct Sampler {
    cpu: cpu::CpuMeter,
    net: network::NetMeter,
}

impl Sampler {
    /// 기준점을 잡기 위해 한 번 샘플링하고 버린다.
    pub fn new(exclude_virtual_adapters: bool) -> Self {
        let mut sampler = Self {
            cpu: cpu::CpuMeter::new(),
            net: network::NetMeter::new(exclude_virtual_adapters),
        };
        sampler.prime();
        sampler
    }

    /// 델타 기준점만 다시 잡는다. 절전 복귀나 폴링 주기 변경 후에 쓴다.
    pub fn prime(&mut self) {
        self.cpu.reset();
        self.net.reset();
        let _ = self.net.sample();
    }

    pub fn sample(&mut self) -> Sample {
        Sample {
            cpu_percent: self.cpu.sample(),
            memory: memory::read(),
            net: self.net.sample(),
        }
    }

    pub fn set_exclude_virtual_adapters(&mut self, exclude: bool) {
        self.net.set_exclude_virtual(exclude);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sampler_returns_real_values() {
        let mut sampler = Sampler::new(true);
        std::thread::sleep(std::time::Duration::from_millis(120));
        let sample = sampler.sample();

        let cpu = sample.cpu_percent.expect("두 번째 샘플부터 값이 나온다");
        assert!((0.0..=100.0).contains(&cpu), "cpu={cpu}");

        let memory = sample.memory.expect("메모리");
        assert!((0.0..=100.0).contains(&memory.percent));

        let net = sample.net.expect("네트워크");
        assert!(net.down >= 0.0 && net.up >= 0.0);
    }

    /// 절전 복귀를 흉내내면 값이 튀지 않고 다시 기준점을 잡아야 한다.
    #[test]
    fn prime_resets_the_baseline() {
        let mut sampler = Sampler::new(true);
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
