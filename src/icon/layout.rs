//! 아이콘 레이아웃 정책.
//!
//! 16px 정사각형 아이콘에 라벨과 값을 2줄로 쌓기 위한 값들은 실측으로 정했다.
//! (docs/SPIKE.md 의 "라벨 + 값 2줄 레이아웃 실측" 참고)

use super::render::Fit;

/// 라벨(작게) + 값(크게) 2줄 레이아웃.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StackSpec {
    /// 라벨 줄 높이(px)
    pub label_h: i32,
    /// 값 줄 높이(px)
    pub value_h: i32,
    /// 두 줄 사이 간격(px)
    pub gap: i32,
    /// 라벨 밝기. 값을 앞세우기 위해 뒤로 물린다.
    pub label_alpha: u8,
    /// 값이 박스 폭을 넘을 때의 처리
    pub value_fit: Fit,
}

impl StackSpec {
    /// 두 줄이 차지하는 전체 높이.
    pub fn block_height(&self) -> i32 {
        self.label_h + self.gap + self.value_h
    }
}

/// 아이콘 크기에 맞는 레이아웃.
///
/// 16px에서 라벨 5 / 값 9 / 간격 1 이 실측 최적값이다.
/// 값은 가로로 압축해 높이를 살린다(비율 유지로 넣으면 6px까지 납작해진다).
pub fn stack_spec(px: i32) -> StackSpec {
    let px = px.max(8);
    let unit = px as f32 / 16.0;
    let mut label_h = (5.0 * unit).round().max(3.0) as i32;
    let mut value_h = (9.0 * unit).round().max(4.0) as i32;
    let gap = if px >= 24 { 2 } else { 1 };

    // 블록이 아이콘보다 커지면 값부터 줄인다.
    while label_h + gap + value_h > px && value_h > 4 {
        value_h -= 1;
    }
    while label_h + gap + value_h > px && label_h > 3 {
        label_h -= 1;
    }

    StackSpec {
        label_h,
        value_h,
        gap,
        label_alpha: 190,
        value_fit: Fit::Squeeze,
    }
}

/// 트레이에 표시하는 지표. 네트워크는 방향별로 아이콘을 나눈다.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Metric {
    Cpu,
    Memory,
    NetworkDown,
    NetworkUp,
}

impl Metric {
    /// 아이콘 생성 순서를 고정한다. 순서가 바뀌면 Windows가 부여하는 아이콘 id가
    /// 바뀌어 사용자의 트레이 승격 설정이 어긋난다.
    ///
    /// v0.1.0의 세 번째 아이콘(Network)은 [Metric::NetworkDown]이 이어받는다.
    /// 그래야 이미 승격해 둔 사용자가 다운로드 아이콘을 계속 본다.
    pub const ALL: [Metric; 4] = [
        Metric::Cpu,
        Metric::Memory,
        Metric::NetworkDown,
        Metric::NetworkUp,
    ];

    /// 아이콘에 넣는 라벨. 16px에서는 3자 이하만 읽힌다.
    pub fn label(self) -> &'static str {
        match self {
            Metric::Cpu => "CPU",
            Metric::Memory => "MEM",
            // 화살표(↓/↑)는 16px(라벨 5px)에서 얇은 선으로 뭉개져 구분되지 않는다.
            // 후보 비교는 docs/PLAN.md 참고.
            Metric::NetworkDown => "DN",
            Metric::NetworkUp => "UP",
        }
    }

    /// 설정과 메뉴에서 쓰는 식별자.
    pub fn id(self) -> &'static str {
        match self {
            Metric::Cpu => "cpu",
            Metric::Memory => "memory",
            Metric::NetworkDown => "network-down",
            Metric::NetworkUp => "network-up",
        }
    }

    pub fn tooltip_name(self) -> &'static str {
        match self {
            Metric::Cpu => "CPU",
            Metric::Memory => "Memory",
            Metric::NetworkDown => "Network down (DN)",
            Metric::NetworkUp => "Network up (UP)",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sixteen_pixels_matches_the_measured_layout() {
        let spec = stack_spec(16);
        assert_eq!(spec.label_h, 5);
        assert_eq!(spec.value_h, 9);
        assert_eq!(spec.gap, 1);
        assert_eq!(spec.block_height(), 15);
        assert_eq!(spec.value_fit, Fit::Squeeze);
    }

    #[test]
    fn block_always_fits_and_value_stays_readable() {
        for px in [8, 12, 16, 20, 24, 32, 40, 48, 64, 96] {
            let spec = stack_spec(px);
            assert!(
                spec.block_height() <= px,
                "px={px} block={}",
                spec.block_height()
            );
            assert!(spec.label_h >= 3, "px={px} label_h={}", spec.label_h);
            assert!(spec.value_h >= 4, "px={px} value_h={}", spec.value_h);
            assert!(
                spec.value_h >= spec.label_h,
                "px={px} 값이 라벨보다 작으면 안 된다"
            );
        }
    }

    #[test]
    fn layout_grows_with_the_icon() {
        let small = stack_spec(16);
        let large = stack_spec(32);
        assert!(large.label_h > small.label_h);
        assert!(large.value_h > small.value_h);
    }

    #[test]
    fn labels_fit_the_three_character_budget() {
        for metric in Metric::ALL {
            assert!(metric.label().chars().count() <= 3, "{:?}", metric);
            assert!(!metric.id().is_empty());
        }
    }

    #[test]
    fn metric_order_is_stable() {
        assert_eq!(
            Metric::ALL.map(|m| m.id()),
            ["cpu", "memory", "network-down", "network-up"],
            "순서가 바뀌면 트레이 승격 설정이 어긋난다"
        );
    }

    /// v0.1.0 의 세 번째 아이콘을 다운로드가 이어받아야 기존 승격이 살아남는다.
    #[test]
    fn network_down_keeps_the_third_icon_slot() {
        assert_eq!(Metric::ALL[2], Metric::NetworkDown);
    }
}
