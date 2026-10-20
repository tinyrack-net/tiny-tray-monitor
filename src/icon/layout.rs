//! 아이콘 레이아웃 정책.
//!
//! 글리프는 16px master 하나뿐이고 배율은 [super::render::scale_nearest]가 처리한다.
//! 여기서는 master 안에서 라벨과 값을 어떻게 쌓을지만 정한다.
//!
//! 값 줄을 [super::pixel::Face::Value](3x9)로 두는 이유는
//! [docs/PIXEL-FONT.md](../../docs/PIXEL-FONT.md) 참고.
//!
//! 라벨을 숨기면 값만 [Face::ValueLarge](4x10), 라벨과 단위를 모두 숨기면
//! 값이 3글자 이하라 [Face::ValueHuge](4x13)로 키운다.

use super::pixel::{Face, TRACKING};

/// 라벨 밝기. 값을 앞세우기 위해 뒤로 물린다.
const LABEL_ALPHA: u8 = 190;

/// 라벨(작게) + 값(크게) 2줄 레이아웃. master(16px) 기준 값이다.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StackSpec {
    /// 라벨 줄 높이(px). 라벨을 숨기면 0.
    pub label_h: i32,
    /// 값 줄 높이(px)
    pub value_h: i32,
    /// 두 줄 사이 간격(px)
    pub gap: i32,
    /// 라벨 밝기. 값을 앞세우기 위해 뒤로 물린다.
    pub label_alpha: u8,
    /// 라벨 줄을 그리는가. false면 값만 크게 그린다.
    pub show_label: bool,
    /// 값 줄에 쓸 글리프 표.
    pub value_face: Face,
}

impl StackSpec {
    /// 두 줄이 차지하는 전체 높이.
    pub fn block_height(&self) -> i32 {
        self.label_h + self.gap + self.value_h
    }
}

/// master 레이아웃. `show_label`과 `show_units`에 따라 결정된다.
///
/// 단위를 숨기면 값이 3글자 이하가 되어 간격 1px을 써도 16px에 들어가므로
/// 가장 큰 글리프를 쓴다.
pub fn stack_spec(show_label: bool, show_units: bool) -> StackSpec {
    let value_face = if show_label {
        Face::Value
    } else if show_units {
        Face::ValueLarge
    } else {
        Face::ValueHuge
    };
    StackSpec {
        label_h: if show_label { Face::Label.height() } else { 0 },
        value_h: value_face.height(),
        gap: if show_label { TRACKING } else { 0 },
        label_alpha: LABEL_ALPHA,
        show_label,
        value_face,
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

    /// 아이콘에 넣는 라벨. 글리프 표는 [super::glyphs::LABEL]에 있다.
    pub fn label(self) -> &'static str {
        match self {
            Metric::Cpu => "CPU",
            Metric::Memory => "MEM",
            // 화살표(↓/↑)는 라벨이 2글자보다 넓고, 색과 함께 보면 DN/UP이 더 명확하다.
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
    use crate::icon::pixel::MASTER_PX;

    #[test]
    fn master_layout_is_the_measured_one() {
        let spec = stack_spec(true, true);
        assert_eq!(spec.label_h, 5);
        assert_eq!(spec.value_h, 9);
        assert_eq!(spec.gap, 1);
        assert_eq!(spec.block_height(), 15);
        assert_eq!(spec.label_alpha, 190);
        assert!(spec.show_label);
        assert_eq!(spec.value_face, Face::Value);
    }

    #[test]
    fn value_only_layout_uses_the_large_face() {
        let spec = stack_spec(false, true);
        assert_eq!(spec.label_h, 0);
        assert_eq!(spec.value_h, 10);
        assert_eq!(spec.gap, 0);
        assert_eq!(spec.block_height(), 10);
        assert!(!spec.show_label);
        assert_eq!(spec.value_face, Face::ValueLarge);
    }

    /// 단위를 숨기면 값이 3글자 이하라 가장 큰 글리프를 쓴다.
    #[test]
    fn unit_less_value_only_layout_uses_the_huge_face() {
        let spec = stack_spec(false, false);
        assert_eq!(spec.label_h, 0);
        assert_eq!(spec.value_h, 13);
        assert_eq!(spec.gap, 0);
        assert_eq!(spec.block_height(), 13);
        assert_eq!(spec.value_face, Face::ValueHuge);
    }

    #[test]
    fn block_fits_the_master() {
        for show_label in [true, false] {
            for show_units in [true, false] {
                let spec = stack_spec(show_label, show_units);
                assert!(
                    spec.block_height() <= MASTER_PX,
                    "block={} master={MASTER_PX}",
                    spec.block_height()
                );
                assert!(spec.value_h >= spec.label_h, "값이 라벨보다 작으면 안 된다");
            }
        }
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
