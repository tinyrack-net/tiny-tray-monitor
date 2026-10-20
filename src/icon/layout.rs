//! 아이콘 레이아웃 정책.
//!
//! 글리프는 16px master 하나뿐이고 배율은 [super::render::scale_nearest]가 처리한다.
//! 여기서는 master 안에서 캡션과 값을 어떻게 쌓을지만 정한다.
//!
//! 아이콘은 캡션(3x5)과 값(4x10) 두 줄이다. 단위는 값 뒤가 아니라 캡션에 둔다.
//! 그래야 값이 항상 3글자 이하라 넓은 숫자를 1px 간격으로 쓸 수 있다.
//! 근거는 [docs/PIXEL-FONT.md](../../docs/PIXEL-FONT.md) §9.

use super::pixel::{Face, TRACKING};

/// 캡션(작게) + 값(크게) 2줄 레이아웃. master(16px) 기준 값이다.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StackSpec {
    /// 캡션 줄 높이(px)
    pub label_h: i32,
    /// 값 줄 높이(px)
    pub value_h: i32,
    /// 두 줄 사이 간격(px)
    pub gap: i32,
    /// 캡션 밝기. 값을 앞세우기 위해 뒤로 물린다.
    pub label_alpha: u8,
}

impl StackSpec {
    /// 두 줄이 차지하는 전체 높이.
    pub const fn block_height(&self) -> i32 {
        self.label_h + self.gap + self.value_h
    }
}

/// master 레이아웃. 캡션 5px + 간격 1px + 값 10px로 16px을 채운다.
pub const STACK: StackSpec = StackSpec {
    label_h: Face::Label.height(),
    value_h: Face::Value.height(),
    gap: TRACKING,
    label_alpha: 190,
};

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

    /// 이 지표가 쓰는 단위. 첫 항목은 아이콘이 처음 뜰 때(첫 표본 전) 쓴다.
    pub fn units(self) -> &'static [&'static str] {
        match self {
            Metric::Cpu | Metric::Memory => &["%"],
            Metric::NetworkDown | Metric::NetworkUp => &["B", "KB", "MB", "GB"],
        }
    }

    /// 아이콘이 처음 뜰 때(첫 표본 전) 쓰는 단위.
    pub fn base_unit(self) -> &'static str {
        self.units()[0]
    }

    /// 아이콘 캡션. 값 줄에는 숫자만 두고 단위는 여기에 둔다.
    ///
    /// 사용률은 `%`가 자명하므로 지표 이름(`CPU`/`MEM`)을 쓰고, 네트워크는 3글자
    /// 안에 방향과 단위를 함께 넣는다(`↓MB`/`↑KB`). 글리프 표는 [super::glyphs::LABEL].
    pub fn caption(self, unit: &str) -> String {
        match self {
            Metric::Cpu => "CPU".to_string(),
            Metric::Memory => "MEM".to_string(),
            Metric::NetworkDown => format!("↓{unit}"),
            Metric::NetworkUp => format!("↑{unit}"),
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
            Metric::NetworkDown => "Network down",
            Metric::NetworkUp => "Network up",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::icon::pixel::MASTER_PX;

    #[test]
    fn layout_fills_the_master() {
        assert_eq!(STACK.label_h, 5);
        assert_eq!(STACK.value_h, 10);
        assert_eq!(STACK.gap, 1);
        assert_eq!(STACK.block_height(), MASTER_PX);
        assert_eq!(STACK.label_alpha, 190);
    }

    #[test]
    fn captions_carry_the_unit() {
        assert_eq!(Metric::Cpu.caption("%"), "CPU");
        assert_eq!(Metric::Memory.caption("%"), "MEM");
        assert_eq!(Metric::NetworkDown.caption("MB"), "↓MB");
        assert_eq!(Metric::NetworkUp.caption("B"), "↑B");
    }

    #[test]
    fn captions_fit_the_three_character_budget() {
        for metric in Metric::ALL {
            for unit in metric.units() {
                let caption = metric.caption(unit);
                assert!(caption.chars().count() <= 3, "{metric:?}: {caption}");
            }
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
