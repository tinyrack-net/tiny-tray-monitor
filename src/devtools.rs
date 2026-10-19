//! 개발용 진단 명령. 배포 바이너리에도 포함되지만 인자를 주지 않으면 실행되지 않는다.

use std::path::Path;

use crate::icon::{self, Bitmap, Color, Fit, GlyphCache, Metric};
use crate::metrics;
use crate::theme;

/// 릴리스 빌드는 `windows_subsystem = "windows"`라 콘솔이 없다.
/// 부모 콘솔에 붙어 println! 출력이 보이게 한다.
pub fn attach_console() {
    use windows::Win32::System::Console::{ATTACH_PARENT_PROCESS, AttachConsole};
    unsafe {
        let _ = AttachConsole(ATTACH_PARENT_PROCESS);
    }
}

/// RGBA 비트맵을 BMP로 저장한다. `zoom`은 정수 배율, 배경색 위에 합성한다.
pub fn write_bmp(path: &str, bitmap: &Bitmap, zoom: i32, background: Color) -> std::io::Result<()> {
    let zoom = zoom.max(1);
    let width = bitmap.width * zoom;
    let height = bitmap.height * zoom;
    let mut data = Vec::with_capacity((width * height * 4) as usize);

    // BMP는 bottom-up BGRA.
    for y in (0..height).rev() {
        for x in 0..width {
            let index = (((y / zoom) * bitmap.width + x / zoom) * 4) as usize;
            let rgba = &bitmap.rgba[index..index + 4];
            let alpha = rgba[3] as u32;
            let blend = |channel: u8, base: u8| -> u8 {
                ((channel as u32 * alpha + base as u32 * (255 - alpha)) / 255) as u8
            };
            data.push(blend(rgba[2], background.2));
            data.push(blend(rgba[1], background.1));
            data.push(blend(rgba[0], background.0));
            data.push(0);
        }
    }

    let mut out = Vec::new();
    let filesize = 14 + 40 + data.len() as u32;
    out.extend_from_slice(b"BM");
    out.extend_from_slice(&filesize.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes());
    out.extend_from_slice(&54u32.to_le_bytes());
    out.extend_from_slice(&40u32.to_le_bytes());
    out.extend_from_slice(&width.to_le_bytes());
    out.extend_from_slice(&height.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&32u16.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes());
    out.extend_from_slice(&data.len().to_le_bytes());
    out.extend_from_slice(&[0u8; 16]);
    out.extend_from_slice(&data);

    if let Some(dir) = Path::new(path).parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(path, out)
}

/// 덤프 파일 이름에 쓰는 짧은 태그. `tools/icon_sheet.py`의 TAGS와 같아야 한다.
fn tag(metric: Metric) -> &'static str {
    match metric {
        Metric::Cpu => "CPU",
        Metric::Memory => "MEM",
        Metric::NetworkDown => "NETD",
        Metric::NetworkUp => "NETU",
    }
}

/// 지표별 대표 표시 값.
fn sample_value(metric: Metric) -> &'static str {
    match metric {
        Metric::Cpu => "80%",
        Metric::Memory => "77%",
        Metric::NetworkDown => "24M",
        Metric::NetworkUp => "1.2M",
    }
}

/// 트레이에 넣을 라벨 후보. 같은 조건으로 렌더해 눈으로 고른다.
const LABEL_CANDIDATES: [(&str, &str); 6] = [
    ("arrow-dn", "↓"),
    ("arrow-up", "↑"),
    ("dn", "DN"),
    ("up", "UP"),
    ("rx", "RX"),
    ("tx", "TX"),
];

/// 아이콘 렌더 결과를 배경별로 덤프하고 실측 글리프 높이를 출력한다.
pub fn dump() {
    const ZOOM: i32 = 16;

    println!(
        "SystemUsesLightTheme = {} -> {} 팔레트",
        theme::is_light(),
        if theme::is_light() {
            "밝은"
        } else {
            "어두운"
        }
    );

    println!(
        "{:<6} {:>8} {:>8} {:>8}  설명",
        "px", "label_h", "value_h", "block"
    );
    for px in [16, 20, 24, 32] {
        let spec = icon::stack_spec(px);
        println!(
            "{:<6} {:>8} {:>8} {:>8}  {}",
            px,
            spec.label_h,
            spec.value_h,
            spec.block_height(),
            if spec.block_height() <= px {
                "ok"
            } else {
                "넘침"
            }
        );
    }

    let backgrounds = [
        ("dark", Color(32, 32, 32), theme::Palette::dark()),
        ("light", Color(243, 243, 243), theme::Palette::light()),
    ];

    for (name, background, palette) in backgrounds {
        let mut cache = GlyphCache::new(GlyphCache::DEFAULT_CAPACITY);
        for px in [16, 24, 32] {
            let spec = icon::stack_spec(px);
            for metric in Metric::ALL {
                let bitmap = icon::stacked(
                    &mut cache,
                    metric.label(),
                    sample_value(metric),
                    px,
                    spec,
                    palette.label,
                    palette.value(metric),
                );
                let path = format!("out/{name}_px{px}_{}@{ZOOM}x.bmp", tag(metric));
                if let Err(error) = write_bmp(&path, &bitmap, ZOOM, background) {
                    eprintln!("{path}: {error}");
                }
            }
        }

        // 16px에서 라벨 후보를 값과 함께 렌더한다.
        let spec = icon::stack_spec(16);
        for (id, label) in LABEL_CANDIDATES {
            let bitmap = icon::stacked(
                &mut cache,
                label,
                "24M",
                16,
                spec,
                palette.label,
                palette.network,
            );
            let path = format!("out/lab_{name}_{id}@{ZOOM}x.bmp");
            if let Err(error) = write_bmp(&path, &bitmap, ZOOM, background) {
                eprintln!("{path}: {error}");
            }
        }
    }

    println!();
    println!("{:<12} {:>9} {:>9}  비고", "라벨", "label_h", "value_h");
    let spec = icon::stack_spec(16);
    let probe = Color(255, 255, 255);
    for (id, label) in LABEL_CANDIDATES
        .iter()
        .copied()
        .chain([("cpu(기준)", "CPU"), ("mem(기준)", "MEM")])
    {
        let label_h = icon::render::render_text(label, 16, spec.label_h, Fit::Uniform, probe, 255)
            .map(|bitmap| bitmap.glyph_h)
            .unwrap_or(0);
        let value_h = icon::render::render_text(
            sample_value(Metric::NetworkDown),
            16,
            spec.value_h,
            spec.value_fit,
            probe,
            255,
        )
        .map(|bitmap| bitmap.glyph_h)
        .unwrap_or(0);
        println!("{:<12} {:>9} {:>9}", id, label_h, value_h);
    }

    println!();
    println!("out/{{dark,light}}_px{{16,24,32}}_{{CPU,MEM,NETD,NETU}}@{ZOOM}x.bmp");
    println!("out/lab_{{dark,light}}_{{arrow-dn,arrow-up,dn,up,rx,tx}}@{ZOOM}x.bmp");
}

/// 1초마다 지표를 출력한다.
pub fn probe() {
    let mut sampler = metrics::Sampler::new(true);
    loop {
        std::thread::sleep(std::time::Duration::from_secs(1));
        let sample = sampler.sample();
        println!(
            "cpu {:>6}  mem {:>6}  net ↓{:>12} ↑{:>12}   아이콘 [{}/{}/{}/{}]",
            sample
                .cpu_percent
                .map(|v| format!("{v:.1}%"))
                .unwrap_or_else(|| "-".into()),
            sample
                .memory
                .map(|m| format!("{:.1}%", m.percent))
                .unwrap_or_else(|| "-".into()),
            sample
                .net
                .map(|n| metrics::format::rate_long(n.down))
                .unwrap_or_else(|| "-".into()),
            sample
                .net
                .map(|n| metrics::format::rate_long(n.up))
                .unwrap_or_else(|| "-".into()),
            sample
                .cpu_percent
                .map(metrics::format::percent)
                .unwrap_or_default(),
            sample
                .memory
                .map(|m| metrics::format::percent(m.percent))
                .unwrap_or_default(),
            sample
                .net
                .map(|n| metrics::format::rate(n.down))
                .unwrap_or_default(),
            sample
                .net
                .map(|n| metrics::format::rate(n.up))
                .unwrap_or_default(),
        );
    }
}

/// 네트워크 인터페이스와 집계 대상 여부를 출력한다.
pub fn interfaces() {
    for exclude_virtual in [true, false] {
        let list = metrics::network::adapters(exclude_virtual);
        println!(
            "exclude_virtual_adapters = {exclude_virtual} (집계 대상 {}개)",
            list.iter().filter(|a| a.counted).count()
        );
        println!(
            "  {:<4} {:<6} {:<6} {:<9} {:<6} description",
            "idx", "type", "up", "hardware", "counted"
        );
        for a in list {
            println!(
                "  {:<4} {:<6} {:<6} {:<9} {:<6} {} ({})",
                a.index, a.if_type, a.up, a.hardware, a.counted, a.description, a.alias
            );
        }
        println!();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 태그가 겹치면 덤프 파일이 서로를 덮어써 조용히 사라진다.
    #[test]
    fn every_metric_has_a_unique_dump_tag() {
        let tags: Vec<&str> = Metric::ALL.iter().map(|metric| tag(*metric)).collect();
        let mut sorted = tags.clone();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.len(), tags.len(), "{tags:?}");
    }

    /// `tools/icon_sheet.py`의 TAGS와 같아야 시트가 만들어진다.
    #[test]
    fn dump_tags_match_the_sheet_tool() {
        let expected = ["CPU", "MEM", "NETD", "NETU"];
        let actual: Vec<&str> = Metric::ALL.iter().map(|metric| tag(*metric)).collect();
        assert_eq!(actual, expected);
    }

    #[test]
    fn every_metric_has_a_sample_value() {
        for metric in Metric::ALL {
            let value = sample_value(metric);
            assert!(!value.is_empty(), "{metric:?}");
            assert!(value.chars().count() <= 4, "{metric:?}: {value}");
        }
    }

    #[test]
    fn label_candidates_are_comparable() {
        assert!(LABEL_CANDIDATES.len() >= 4);
        for (id, label) in LABEL_CANDIDATES {
            assert!(!id.is_empty() && !label.is_empty());
        }
    }
}
