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

/// 아이콘 크기를 정하는 DPI 값들을 출력한다. `--watch`면 1초마다 바뀐 줄만 다시 찍는다.
///
/// 실행 중에 배율을 바꿔 가며 어느 값이 따라가는지 보는 용도다(`docs/SPIKE.md` §16).
pub fn dpi(watch: bool) {
    let mut last = String::new();
    loop {
        let report = crate::win32::dpi_report();
        let line = format!(
            "system dpi {:>3}  taskbar dpi {:>4}  GetSystemMetrics(SM_CXSMICON) {:>2}px  -> icon {:>2}px ({})",
            report.system_dpi,
            report
                .taskbar_dpi
                .map(|dpi| dpi.to_string())
                .unwrap_or_else(|| "-".into()),
            report.legacy_px,
            report.px,
            icon::render::fit_label(report.px),
        );
        if line != last {
            println!("{line}");
            last = line;
        }
        if !watch {
            return;
        }
        std::thread::sleep(std::time::Duration::from_secs(1));
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
        Metric::Gpu => "GPU",
        Metric::GpuVram => "VRM",
        Metric::DiskUsage => "DSK",
        Metric::DiskActive => "DIO",
    }
}

/// 지표별 대표 표시 값(숫자, 단위).
fn sample_value(metric: Metric) -> (&'static str, &'static str) {
    match metric {
        Metric::Cpu => ("80", "%"),
        Metric::Memory => ("77", "%"),
        Metric::NetworkDown => ("24", "MB"),
        Metric::NetworkUp => ("1.2", "MB"),
        Metric::Gpu => ("14", "%"),
        Metric::GpuVram => ("75", "%"),
        Metric::DiskUsage => ("58", "%"),
        Metric::DiskActive => ("100", "%"),
    }
}

/// 넘침을 확인할 가장 넓은 값 후보(숫자 3글자, 가장 넓은 캡션).
const LONG_VALUES: [(&str, Metric, &str, &str); 3] = [
    ("cpu100", Metric::Cpu, "100", "%"),
    ("dn1.2gb", Metric::NetworkDown, "1.2", "GB"),
    ("dn999kb", Metric::NetworkDown, "999", "KB"),
];

/// 배율이 master(16px)의 정수배인지 표시한다.
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

/// 덤프하는 아이콘 크기. 100~300% 배율 중 시트로 판정할 것들이다(`Makefile`의 `dump`와 맞춘다).
const DUMP_SIZES: [i32; 8] = [16, 20, 24, 28, 32, 36, 40, 48];

/// 아이콘 렌더 결과를 배경별·px별로 덤프하고 1비트 여부를 출력한다.
pub fn dump() {
    const ZOOM: i32 = 16;

    println!(
        "SystemUsesLightTheme = {} -> {} palette",
        theme::is_light(),
        if theme::is_light() { "light" } else { "dark" }
    );

    let backgrounds = [
        ("dark", Color(32, 32, 32), theme::Palette::dark()),
        ("light", Color(243, 243, 243), theme::Palette::light()),
    ];

    for (name, background, palette) in backgrounds {
        let mut cache = GlyphCache::new(GlyphCache::DEFAULT_CAPACITY);
        let mut save = |file: String, metric: Metric, caption: String, number: &str, px: i32| {
            let bitmap = icon::stacked(
                &mut cache,
                &caption,
                number,
                px,
                palette.label,
                palette.value(metric),
            );
            let path = format!("out/{name}_{file}@{ZOOM}x.bmp");
            if let Err(error) = write_bmp(&path, &bitmap, ZOOM, background) {
                eprintln!("{path}: {error}");
            }
        };

        for px in DUMP_SIZES {
            for metric in Metric::ALL {
                let (number, unit) = sample_value(metric);
                save(
                    format!("px{px}_{}", tag(metric)),
                    metric,
                    metric.caption(unit),
                    number,
                    px,
                );
            }
        }

        // 가장 넓은 값들이라도 16px master에 들어가는지 확인한다.
        for (id, metric, number, unit) in LONG_VALUES {
            save(
                format!("long_{id}"),
                metric,
                metric.caption(unit),
                number,
                16,
            );
        }
    }

    // 배율별로 블록이 들어가고 1비트(계조 0/캡션/값)를 유지하는지 값으로 확인한다.
    let block = icon::STACK.block_height();
    println!();
    println!("{:<6} {:>10} {:>6}  alpha levels", "px", "scale", "block");
    let mut cache = GlyphCache::new(GlyphCache::DEFAULT_CAPACITY);
    let white = Color(255, 255, 255);
    for px in [16, 20, 24, 28, 32, 36, 40, 44, 48, 52, 56] {
        let bitmap = icon::stacked(
            &mut cache,
            &Metric::NetworkDown.caption("MB"),
            "100",
            px,
            white,
            white,
        );
        println!(
            "{:<6} {:>10} {:>6}  {:?}{}",
            px,
            icon::render::fit_label(px),
            block,
            alpha_levels(&bitmap),
            if block <= icon::pixel::MASTER_PX {
                ""
            } else {
                "  overflow"
            }
        );
    }

    println!();
    println!(
        "out/{{dark,light}}_px{{{}}}_{{CPU,MEM,NETD,NETU,GPU,VRM,DSK,DIO}}@{ZOOM}x.bmp",
        DUMP_SIZES.map(|px| px.to_string()).join(",")
    );
    println!("out/{{dark,light}}_long_{{cpu100,dn1.2gb,dn999kb}}@{ZOOM}x.bmp");
}

/// 1초마다 지표를 출력한다.
pub fn probe() {
    let config = crate::config::Store::load().config;
    let mut sampler = metrics::Sampler::new(&config.devices);
    loop {
        std::thread::sleep(std::time::Duration::from_secs(1));
        let sample = sampler.sample();
        println!(
            "cpu {:>6}  mem {:>6}  net down {:>12} up {:>12}  gpu {:>6}  vram {:>6}  disk {:>6}  io {:>6} (r {:>12} w {:>12})   icons [{}/{}/{}/{}/{}/{}/{}/{}]",
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
                .gpu_percent
                .map(|g| format!("{g:.1}%"))
                .unwrap_or_else(|| "-".into()),
            sample
                .gpu_vram
                .map(|v| format!("{:.1}%", v.percent))
                .unwrap_or_else(|| "-".into()),
            sample
                .disk_usage
                .map(|d| format!("{:.1}%", d.percent))
                .unwrap_or_else(|| "-".into()),
            sample
                .disk_active
                .map(|d| format!("{:.1}%", d.percent))
                .unwrap_or_else(|| "-".into()),
            sample
                .disk_active
                .map(|d| metrics::format::rate_long(d.read))
                .unwrap_or_else(|| "-".into()),
            sample
                .disk_active
                .map(|d| metrics::format::rate_long(d.write))
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
                .map(|n| metrics::format::rate_reading(n.down).compact())
                .unwrap_or_default(),
            sample
                .net
                .map(|n| metrics::format::rate_reading(n.up).compact())
                .unwrap_or_default(),
            sample
                .gpu_percent
                .map(metrics::format::percent)
                .unwrap_or_default(),
            sample
                .gpu_vram
                .map(|v| metrics::format::percent(v.percent))
                .unwrap_or_default(),
            sample
                .disk_usage
                .map(|d| metrics::format::percent(d.percent))
                .unwrap_or_default(),
            sample
                .disk_active
                .map(|d| metrics::format::percent(d.percent))
                .unwrap_or_default(),
        );
    }
}

/// 네트워크 인터페이스와 집계 대상 여부를 출력한다.
pub fn interfaces() {
    let list = metrics::network::adapters();
    println!(
        "{} counted (All adapters)",
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
        let expected = ["CPU", "MEM", "NETD", "NETU", "GPU", "VRM", "DSK", "DIO"];
        let actual: Vec<&str> = Metric::ALL.iter().map(|metric| tag(*metric)).collect();
        assert_eq!(actual, expected);
    }

    #[test]
    fn every_metric_has_a_sample_value() {
        for metric in Metric::ALL {
            let (number, unit) = sample_value(metric);
            assert!(!number.is_empty(), "{metric:?}");
            assert!(number.chars().count() <= 3, "{metric:?}: {number}");
            assert!(!unit.is_empty());
        }
    }

    /// 가장 넓은 값이 16px master에 들어간다.
    #[test]
    fn long_values_fit_the_master() {
        use icon::pixel::{MASTER_PX, measure};
        for (id, metric, number, unit) in LONG_VALUES {
            assert!(!id.is_empty());
            assert_eq!(number.chars().count(), 3, "{number}");
            assert!(measure(&metric.caption(unit), icon::Face::Label) <= MASTER_PX);
            assert!(measure(number, icon::Face::Value) <= MASTER_PX);
        }
    }

    #[test]
    fn fit_labels_distinguish_integer_multiples() {
        use crate::icon::render::fit_label;
        assert_eq!(fit_label(16), "1x");
        assert_eq!(fit_label(32), "2x");
        assert_eq!(fit_label(36), "2x+2");
        assert!(fit_label(20).starts_with("NN"));
        assert!(fit_label(24).starts_with("NN"));
    }

    #[test]
    fn alpha_levels_reports_each_distinct_alpha() {
        let mut bitmap = Bitmap::transparent(2, 1);
        bitmap.set(0, 0, Color(0, 0, 0), 190);
        bitmap.set(1, 0, Color(0, 0, 0), 255);
        assert_eq!(alpha_levels(&bitmap), vec![190, 255]);
    }
}
