//! 개발용 진단 명령. 배포 바이너리에도 포함되지만 인자를 주지 않으면 실행되지 않는다.

use std::path::Path;

use crate::icon::{self, Bitmap, Color, GlyphCache, Metric};
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

/// 트레이가 따르는 테마와 다크 모드 API 지원 여부. 데스크톱이 없어도 된다.
pub fn theme() {
    let palette = theme::current();
    println!(
        "taskbar theme : {}",
        if palette.light { "light" } else { "dark" }
    );
    println!(
        "dark mode API : {}",
        if crate::win32::dark::available() {
            "available (uxtheme ordinals 133/135/136)"
        } else {
            "missing (uxtheme ordinals not found)"
        }
    );

    // 실제로 걸어 봐서 예외 없이 지나가는지까지 본다. 프로세스는 곧 끝난다.
    crate::win32::dark::set_app_mode(!palette.light);
    println!(
        "app mode      : {}",
        if palette.light {
            "ForceLight"
        } else {
            "ForceDark"
        }
    );
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
fn sample_value(metric: Metric, units: bool) -> &'static str {
    match (metric, units) {
        (Metric::Cpu, true) => "80%",
        (Metric::Memory, true) => "77%",
        (Metric::NetworkDown, true) => "24M",
        (Metric::NetworkUp, true) => "1.2M",
        (Metric::Cpu, false) => "80",
        (Metric::Memory, false) => "77",
        (Metric::NetworkDown, false) => "24",
        (Metric::NetworkUp, false) => "1.2",
    }
}

/// 넘침을 확인할 4글자 값 후보.
const LONG_VALUES: [(&str, Metric, &str); 3] = [
    ("cpu100", Metric::Cpu, "100%"),
    ("dn1.2g", Metric::NetworkDown, "1.2G"),
    ("dn999k", Metric::NetworkDown, "999K"),
];

/// 단위를 숨겼을 때의 가장 넓은 값(3글자). 확대 글리프에 들어가는지 확인한다.
const WIDE_UNIT_LESS: [(&str, Metric, &str); 3] = [
    ("cpu100", Metric::Cpu, "100"),
    ("dn1.2", Metric::NetworkDown, "1.2"),
    ("dn999", Metric::NetworkDown, "999"),
];

/// 배율이 master(16px)의 정수배인지 표시한다.
fn scale_label(px: i32) -> String {
    let master = icon::pixel::MASTER_PX;
    if px % master == 0 {
        format!("{}x", px / master)
    } else {
        format!("NN {:.2}x", px as f32 / master as f32)
    }
}

/// 알파 계조(0 제외). 1비트면 값이 하나뿐이어야 한다.
fn alpha_levels(bitmap: &Bitmap) -> Vec<u8> {
    let mut levels: Vec<u8> = bitmap
        .rgba
        .chunks_exact(4)
        .map(|pixel| pixel[3])
        .filter(|alpha| *alpha > 0)
        .collect();
    levels.sort_unstable();
    levels.dedup();
    levels
}

/// 아이콘 렌더 결과를 배경별·px별로 덤프하고 1비트 여부를 출력한다.
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

    let spec = icon::stack_spec(true, true);
    println!();
    println!("{:<6} {:>10} {:>8}  비고", "px", "배율", "block");
    for px in [16, 20, 24, 28, 32, 48] {
        println!(
            "{:<6} {:>10} {:>8}  {}",
            px,
            scale_label(px),
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
        // 라벨/단위 조합 네 가지를 모두 덤프한다.
        for (prefix, show_label, show_units) in [
            ("px", true, true),
            ("nounit_px", true, false),
            ("nolabel_px", false, true),
            ("nolabel_nounit_px", false, false),
        ] {
            let spec = icon::stack_spec(show_label, show_units);
            for px in [16, 20, 24, 32] {
                for metric in Metric::ALL {
                    let bitmap = icon::stacked(
                        &mut cache,
                        metric.label(),
                        sample_value(metric, show_units),
                        px,
                        spec,
                        palette.label,
                        palette.value(metric),
                    );
                    let path = format!("out/{name}_{prefix}{px}_{}@{ZOOM}x.bmp", tag(metric));
                    if let Err(error) = write_bmp(&path, &bitmap, ZOOM, background) {
                        eprintln!("{path}: {error}");
                    }
                }
            }
        }

        // 가장 넓은 값들이라도 16px master에 들어가는지 확인한다.
        for (id, metric, value) in LONG_VALUES {
            for (prefix, show_label) in [("long", true), ("nolabel_long", false)] {
                let spec = icon::stack_spec(show_label, true);
                let bitmap = icon::stacked(
                    &mut cache,
                    metric.label(),
                    value,
                    16,
                    spec,
                    palette.label,
                    palette.value(metric),
                );
                let path = format!("out/{name}_{prefix}_{id}@{ZOOM}x.bmp");
                if let Err(error) = write_bmp(&path, &bitmap, ZOOM, background) {
                    eprintln!("{path}: {error}");
                }
            }
        }

        // 단위를 숨기면 3글자 이하라 확대 글리프에도 들어간다.
        for (id, metric, value) in WIDE_UNIT_LESS {
            let spec = icon::stack_spec(false, false);
            let bitmap = icon::stacked(
                &mut cache,
                metric.label(),
                value,
                16,
                spec,
                palette.label,
                palette.value(metric),
            );
            let path = format!("out/{name}_nounit_long_{id}@{ZOOM}x.bmp");
            if let Err(error) = write_bmp(&path, &bitmap, ZOOM, background) {
                eprintln!("{path}: {error}");
            }
        }
    }

    // 배율별로 1비트(계조 0/라벨/값)를 유지하는지 눈이 아니라 값으로 확인한다.
    println!();
    println!(
        "{:<6} {:>10} {:>16}  라벨+단위 / 라벨만 / 값만 / 값만+단위없음",
        "px", "배율", "block"
    );
    let mut cache = GlyphCache::new(GlyphCache::DEFAULT_CAPACITY);
    let white = Color(255, 255, 255);
    for px in [16, 20, 24, 28, 32, 48] {
        let render = |cache: &mut GlyphCache, label: bool, units: bool, value: &str| {
            icon::stacked(
                cache,
                "CPU",
                value,
                px,
                icon::stack_spec(label, units),
                white,
                white,
            )
        };
        let labeled = render(&mut cache, true, true, "100%");
        let label_only = render(&mut cache, true, false, "100");
        let value_only = render(&mut cache, false, true, "100%");
        let unit_less = render(&mut cache, false, false, "100");
        println!(
            "{:<6} {:>10} {:>16}  {:?} / {:?} / {:?} / {:?}",
            px,
            scale_label(px),
            spec.block_height(),
            alpha_levels(&labeled),
            alpha_levels(&label_only),
            alpha_levels(&value_only),
            alpha_levels(&unit_less)
        );
    }

    println!();
    println!("out/{{dark,light}}_px{{16,20,24,32}}_{{CPU,MEM,NETD,NETU}}@{ZOOM}x.bmp");
    println!("out/{{dark,light}}_nounit_px{{16,20,24,32}}_{{CPU,MEM,NETD,NETU}}@{ZOOM}x.bmp");
    println!("out/{{dark,light}}_nolabel_px{{16,20,24,32}}_{{CPU,MEM,NETD,NETU}}@{ZOOM}x.bmp");
    println!(
        "out/{{dark,light}}_nolabel_nounit_px{{16,20,24,32}}_{{CPU,MEM,NETD,NETU}}@{ZOOM}x.bmp"
    );
    println!("out/{{dark,light}}_{{long,nolabel_long}}_{{cpu100,dn1.2g,dn999k}}@{ZOOM}x.bmp");
    println!("out/{{dark,light}}_nounit_long_{{cpu100,dn1.2,dn999}}@{ZOOM}x.bmp");
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
            for units in [true, false] {
                let value = sample_value(metric, units);
                assert!(!value.is_empty(), "{metric:?}");
                let budget = if units { 4 } else { 3 };
                assert!(value.chars().count() <= budget, "{metric:?}: {value}");
            }
        }
    }

    #[test]
    fn long_values_are_the_widest_real_ones() {
        for (id, _, value) in LONG_VALUES {
            assert!(!id.is_empty());
            assert_eq!(value.chars().count(), 4, "{value}");
        }
    }

    /// 단위를 숨긴 값은 3글자 이하라 확대 글리프에 들어간다.
    #[test]
    fn unit_less_values_fit_the_huge_face() {
        for (id, _, value) in WIDE_UNIT_LESS {
            assert!(!id.is_empty());
            assert_eq!(value.chars().count(), 3, "{value}");
            assert!(icon::pixel::measure(value, icon::Face::ValueHuge) <= icon::pixel::MASTER_PX);
        }
    }

    #[test]
    fn scale_labels_distinguish_integer_multiples() {
        assert_eq!(scale_label(16), "1x");
        assert_eq!(scale_label(32), "2x");
        assert!(scale_label(20).starts_with("NN"));
        assert!(scale_label(24).starts_with("NN"));
    }

    #[test]
    fn alpha_levels_reports_each_distinct_alpha() {
        let mut bitmap = Bitmap::transparent(2, 1);
        bitmap.set(0, 0, Color(0, 0, 0), 190);
        bitmap.set(1, 0, Color(0, 0, 0), 255);
        assert_eq!(alpha_levels(&bitmap), vec![190, 255]);
    }
}
