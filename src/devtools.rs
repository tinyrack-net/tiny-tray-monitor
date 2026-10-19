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

/// RGBA 비트맵을 BMP로 저장한다. `zoom`은 정수 배율, 배경색 위에 합성한다.
pub fn write_bmp(path: &str, bitmap: &Bitmap, zoom: i32, background: Color) -> std::io::Result<()> {
    let zoom = zoom.max(1);
    let width = bitmap.width * zoom;
    let height = bitmap.height * zoom;
    let mut data = Vec::with_capacity((width * height * 4) as usize);

    // BMP는 bottom-up BGRA.
    for y in (0..height).rev() {
        for x in 0..width {
            let rgba =
                bitmap.rgba[((y / zoom) * bitmap.width + x / zoom) as usize * 4..][..4].to_vec();
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
    let values = [("CPU", "80%"), ("MEM", "77%"), ("NET", "24M")];
    let backgrounds = [
        ("dark", Color(32, 32, 32), theme::Palette::dark()),
        ("light", Color(243, 243, 243), theme::Palette::light()),
    ];

    println!("{:<6} {:>8} {:>8} {:>8}  설명", "px", "label_h", "value_h", "block");
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

    for (name, background, palette) in backgrounds {
        let mut cache = GlyphCache::new(GlyphCache::DEFAULT_CAPACITY);
        for px in [16, 24, 32] {
            let spec = icon::stack_spec(px);
            for (label, value) in values {
                let metric = match label {
                    "CPU" => Metric::Cpu,
                    "MEM" => Metric::Memory,
                    _ => Metric::Network,
                };
                let bitmap = icon::stacked(
                    &mut cache,
                    metric,
                    value,
                    px,
                    spec,
                    palette.label,
                    palette.value(metric),
                );
                let path = format!("out/{name}_px{px}_{label}@{ZOOM}x.bmp");
                if let Err(error) = write_bmp(&path, &bitmap, ZOOM, background) {
                    eprintln!("{path}: {error}");
                }
            }
        }
    }
    println!("out/<테마>_px<크기>_<지표>@{ZOOM}x.bmp 덤프 완료");
}

/// 1초마다 지표를 출력한다.
pub fn probe() {
    let mut sampler = metrics::Sampler::new(true);
    loop {
        std::thread::sleep(std::time::Duration::from_secs(1));
        let sample = sampler.sample();
        println!(
            "cpu {:>6}  mem {:>6}  net ↓{:>12} ↑{:>12}   아이콘 [{}/{}/{}]",
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
