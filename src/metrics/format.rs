//! 값 -> 표시 문자열 변환.
//!
//! 트레이 아이콘은 정사각형이고 폭이 16~24px이므로 표시 문자열을 최대 4자로 제한한다.
//! 아이콘은 숫자와 단위를 나눠 그린다([Reading]). 단위는 캡션 줄로 올라가고 값 줄에는
//! 3자 이하의 숫자만 남는다. 모두 순수 함수라 단위 테스트로 검증한다.

const UNITS: [&str; 4] = ["", "K", "M", "G"];

/// 아이콘 캡션에 쓰는 전송률 단위. [UNITS]와 같은 순서다.
const RATE_UNITS: [&str; 4] = ["B", "KB", "MB", "GB"];

/// 아이콘에 그리는 값. 숫자는 값 줄에, 단위는 캡션 줄에 들어간다.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reading {
    /// 숫자만(`80`, `1.2`, `512`). 최대 3자.
    pub number: String,
    /// 단위(`%`, `B`, `KB`, `MB`, `GB`).
    pub unit: &'static str,
}

impl Reading {
    /// 로그용 한 줄 표기. (예: "1.2MB")
    pub fn compact(&self) -> String {
        format!("{}{}", self.number, self.unit)
    }
}

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
    percent_reading(value).compact()
}

fn percent_number(value: f64) -> i64 {
    if !value.is_finite() || value <= 0.0 {
        return 0;
    }
    let v = value.min(100.0);
    if v >= 99.5 { 100 } else { v.round() as i64 }
}

/// 사용률을 아이콘 값으로. 숫자는 최대 3자("100").
pub fn percent_reading(value: f64) -> Reading {
    Reading {
        number: percent_number(value).to_string(),
        unit: "%",
    }
}

/// 초당 바이트를 (숫자, 단위 단계)로 압축한다. 숫자는 최대 3자.
fn rate_number(bytes_per_sec: f64) -> (String, usize) {
    let v = if bytes_per_sec.is_finite() {
        bytes_per_sec.max(0.0)
    } else {
        0.0
    };
    let (scaled, unit) = scale(v, 999.5);
    let number = if unit > 0 && scaled < 9.95 {
        format!("{scaled:.1}")
    } else {
        (scaled.round() as i64).to_string()
    };
    (number, unit)
}

/// 초당 바이트를 아이콘 값으로. ("24" + "MB", "512" + "B")
pub fn rate_reading(bytes_per_sec: f64) -> Reading {
    let (number, unit) = rate_number(bytes_per_sec);
    Reading {
        number,
        unit: RATE_UNITS[unit],
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
    use crate::icon::Metric;

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
        let rate = |v: f64| rate_reading(v).compact();
        assert_eq!(rate(0.0), "0B");
        assert_eq!(rate(512.0), "512B");
        assert_eq!(rate(1023.0), "1.0KB");
        assert_eq!(rate(K), "1.0KB");
        assert_eq!(rate(24.0 * K), "24KB");
        assert_eq!(rate(512.0 * K), "512KB");
        assert_eq!(rate(K * K * 1.2), "1.2MB");
        assert_eq!(rate(24.0 * K * K), "24MB");
        assert_eq!(rate(1.2 * K * K * K), "1.2GB");
        // 비정상 입력은 0으로 낮춘다 (표시가 튀는 것보다 낫다)
        assert_eq!(rate(f64::INFINITY), "0B");
        assert_eq!(rate(f64::NAN), "0B");
        assert_eq!(rate(-5.0), "0B");
    }

    /// 아이콘 값 줄에는 3자 이하의 숫자만 들어간다.
    #[test]
    fn readings_keep_numbers_to_three_characters() {
        let mut v = 0.0f64;
        while v < 10.0 * 1024.0 * 1024.0 * 1024.0 {
            let reading = rate_reading(v);
            assert!(reading.number.chars().count() <= 3, "rate_reading({v})");
            assert!(
                Metric::NetworkDown.units().contains(&reading.unit),
                "캡션 글리프 검사는 Metric::units()를 기준으로 한다: {}",
                reading.unit
            );
            assert!(
                reading
                    .number
                    .chars()
                    .all(|c| c.is_ascii_digit() || c == '.')
            );
            v = v * 1.07 + 1.0;
        }
        for i in 0..=1000 {
            let reading = percent_reading(i as f64 / 10.0);
            assert!(reading.number.chars().count() <= 3);
            assert!(Metric::Cpu.units().contains(&reading.unit));
        }
    }

    #[test]
    fn readings_split_number_and_unit() {
        const K: f64 = 1024.0;
        let pair = |r: Reading| (r.number, r.unit);
        assert_eq!(pair(percent_reading(80.4)), ("80".into(), "%"));
        assert_eq!(pair(percent_reading(100.0)), ("100".into(), "%"));
        assert_eq!(pair(rate_reading(0.0)), ("0".into(), "B"));
        assert_eq!(pair(rate_reading(512.0)), ("512".into(), "B"));
        assert_eq!(pair(rate_reading(K)), ("1.0".into(), "KB"));
        assert_eq!(pair(rate_reading(24.0 * K * K)), ("24".into(), "MB"));
        assert_eq!(pair(rate_reading(1.2 * K * K * K)), ("1.2".into(), "GB"));
        assert_eq!(rate_reading(24.0 * K * K).compact(), "24MB");
    }

    #[test]
    fn long_forms() {
        assert_eq!(rate_long(1536.0), "1.50 KB/s");
        assert_eq!(bytes(1024 * 1024), "1.00 MB");
        assert_eq!(bytes(512), "512 B");
    }
}
