//! 값 -> 표시 문자열 변환.
//!
//! 트레이 아이콘은 정사각형이고 폭이 16~24px이므로 표시 문자열을 최대 4자로 제한한다.
//! 모두 순수 함수라 단위 테스트로 검증한다.

const UNITS: [&str; 4] = ["", "K", "M", "G"];

/// `limit`을 넘으면 다음 단위로 올린다.
fn scale(mut value: f64, limit: f64) -> (f64, usize) {
    let mut unit = 0;
    while value >= limit && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    (value, unit)
}

/// 사용률(%)을 표시 문자열로. 최대 4자("100%").
pub fn percent(value: f64) -> String {
    percent_unit(value, true)
}

/// 사용률을 표시 문자열로. `units`가 false면 `%`를 뺀다(최대 3자).
pub fn percent_unit(value: f64, units: bool) -> String {
    if !value.is_finite() || value <= 0.0 {
        return if units {
            "0%".to_string()
        } else {
            "0".to_string()
        };
    }
    let v = value.min(100.0);
    let number = if v >= 99.5 { 100 } else { v.round() as i64 };
    if units {
        format!("{number}%")
    } else {
        number.to_string()
    }
}

/// 초당 바이트를 최대 4자로 압축한다. ("512", "1.0K", "24M", "1.2G")
pub fn rate(bytes_per_sec: f64) -> String {
    rate_unit(bytes_per_sec, true)
}

/// 초당 바이트를 압축한다. `units`가 false면 K/M/G 접미사를 뺀다(최대 3자).
pub fn rate_unit(bytes_per_sec: f64, units: bool) -> String {
    let v = if bytes_per_sec.is_finite() {
        bytes_per_sec.max(0.0)
    } else {
        0.0
    };
    let (scaled, unit) = scale(v, 999.5);
    let suffix = if units { UNITS[unit] } else { "" };
    if unit > 0 && scaled < 9.95 {
        format!("{scaled:.1}{suffix}")
    } else {
        format!("{}{suffix}", scaled.round() as i64)
    }
}

/// 툴팁용 상세 표기. (예: "1.23 MB/s")
pub fn rate_long(bytes_per_sec: f64) -> String {
    let v = if bytes_per_sec.is_finite() {
        bytes_per_sec.max(0.0)
    } else {
        0.0
    };
    let (scaled, unit) = scale(v, 1024.0);
    if unit == 0 {
        format!("{scaled:.0} B/s")
    } else {
        format!("{scaled:.2} {}B/s", UNITS[unit])
    }
}

/// 툴팁용 바이트 표기. (예: "10.09 GB")
pub fn bytes(value: u64) -> String {
    let (scaled, unit) = scale(value as f64, 1024.0);
    if unit == 0 {
        format!("{scaled:.0} B")
    } else {
        format!("{scaled:.2} {}B", UNITS[unit])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn percent_rounds_and_clamps() {
        assert_eq!(percent(-1.0), "0%");
        assert_eq!(percent(0.0), "0%");
        assert_eq!(percent(0.4), "0%");
        assert_eq!(percent(80.4), "80%");
        assert_eq!(percent(99.4), "99%");
        assert_eq!(percent(99.5), "100%");
        assert_eq!(percent(100.0), "100%");
        assert_eq!(percent(500.0), "100%");
        assert_eq!(percent(f64::NAN), "0%");
    }

    #[test]
    fn rate_examples() {
        const K: f64 = 1024.0;
        assert_eq!(rate(0.0), "0");
        assert_eq!(rate(512.0), "512");
        assert_eq!(rate(1023.0), "1.0K");
        assert_eq!(rate(K), "1.0K");
        assert_eq!(rate(24.0 * K), "24K");
        assert_eq!(rate(512.0 * K), "512K");
        assert_eq!(rate(K * K * 1.2), "1.2M");
        assert_eq!(rate(24.0 * K * K), "24M");
        assert_eq!(rate(1.2 * K * K * K), "1.2G");
        // 비정상 입력은 0으로 낮춘다 (표시가 튀는 것보다 낫다)
        assert_eq!(rate(f64::INFINITY), "0");
        assert_eq!(rate(f64::NAN), "0");
        assert_eq!(rate(-5.0), "0");
    }

    /// 값이 어떤 범위여도 아이콘에 들어가도록 4자를 넘지 않아야 한다.
    #[test]
    fn rate_never_exceeds_four_chars() {
        let mut v = 0.0f64;
        while v < 10.0 * 1024.0 * 1024.0 * 1024.0 {
            let s = rate(v);
            assert!(s.chars().count() <= 4, "rate({v}) = {s}");
            v = v * 1.07 + 1.0;
        }
    }

    #[test]
    fn percent_never_exceeds_four_chars() {
        for i in 0..=1000 {
            let s = percent(i as f64 / 10.0);
            assert!(s.chars().count() <= 4, "percent({i}) = {s}");
        }
    }

    #[test]
    fn unit_less_forms_drop_the_suffix() {
        assert_eq!(percent_unit(80.4, false), "80");
        assert_eq!(percent_unit(100.0, false), "100");
        assert_eq!(percent_unit(-1.0, false), "0");
        assert_eq!(rate_unit(1024.0, false), "1.0");
        assert_eq!(rate_unit(24.0 * 1024.0, false), "24");
        assert_eq!(rate_unit(512.0, false), "512");
        assert_eq!(rate_unit(0.0, false), "0");
    }

    /// 단위를 빼면 3자를 넘지 않아야 확대 글리프에 들어간다.
    #[test]
    fn unit_less_forms_never_exceed_three_chars() {
        let mut v = 0.0f64;
        while v < 10.0 * 1024.0 * 1024.0 * 1024.0 {
            assert!(rate_unit(v, false).chars().count() <= 3, "rate_unit({v})");
            v = v * 1.07 + 1.0;
        }
        for i in 0..=1000 {
            assert!(percent_unit(i as f64 / 10.0, false).chars().count() <= 3);
        }
    }

    #[test]
    fn long_forms() {
        assert_eq!(rate_long(1536.0), "1.50 KB/s");
        assert_eq!(bytes(1024 * 1024), "1.00 MB");
        assert_eq!(bytes(512), "512 B");
    }
}
