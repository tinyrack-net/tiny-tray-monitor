//! 트레이 시스템 모니터.
//!
//! CPU / 메모리 / 네트워크 상태를 Windows 트레이 아이콘 3개에 표시한다.
//! 진단용 명령은 [devtools] 참고.
//!
//! ## 설계 제약 (실측 근거는 docs/SPIKE.md)
//!
//! - 아이콘은 생성 순서로 id가 부여되고 승격 설정이 그 id에 묶이므로 재생성하지 않는다.
//! - 빈 문자열을 GDI에 넘기면 프로세스가 죽는다. 렌더 진입점에서 막는다.
//! - 타이머를 스레드 큐로 받으면 트레이 메뉴가 열린 동안 갱신이 멈춘다. 숨은 창에서 받는다.
//! - 폰트/DC를 매번 새로 만들지 않는다. 글리프 캐시를 쓴다.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod config;
mod devtools;
mod icon;
mod ipc;
mod logging;
mod metrics;
mod platform;
mod runtime;
mod settings;
mod startup;
mod theme;
mod timer;
mod tray;
mod win32;

use windows::Win32::UI::WindowsAndMessaging::{
    DispatchMessageW, GetMessageW, MSG, TranslateMessage,
};

/// 릴리스 빌드는 `panic = "abort"`라 패닉이 그냥 프로세스 종료로 보인다.
/// 원인을 로그에 남겨야 현장에서 진단할 수 있다.
fn install_panic_hook() {
    std::panic::set_hook(Box::new(|info| {
        logging::error(format!("panic: {info}"));
    }));
}

/// 사용자에게 보이는 제품 이름. Store 표시 이름과 맞춘다.
/// exe 이름과 설치·설정 경로는 업데이트 호환을 위해 `tray-monitor`로 유지한다.
pub const PRODUCT_NAME: &str = "TinyTrayManager";

const USAGE: &str = "\
TinyTrayManager - CPU / memory / network readouts in the Windows tray

usage:
  tray-monitor                run the tray app
  tray-monitor --settings     run and open the settings window
  tray-monitor --probe        print metrics every second (no tray)
  tray-monitor --interfaces   list network interfaces and what is counted
  tray-monitor --dump         dump icon renderings and layout measurements
  tray-monitor --theme        print the taskbar theme and dark mode support
  tray-monitor --help         show this message
";

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let has = |flag: &str| args.iter().any(|a| a == flag);
    let wants_settings = has("--settings");

    // 진단 명령은 릴리스 빌드에도 콘솔이 없으므로 부모 콘솔에 붙는다.
    if has("--dump") || has("--probe") || has("--interfaces") || has("--theme") || has("--help") {
        devtools::attach_console();
    }
    if has("--theme") {
        devtools::theme();
        return;
    }
    if has("--dump") {
        devtools::dump();
        return;
    }
    if has("--probe") {
        devtools::probe();
        return;
    }
    if has("--interfaces") {
        devtools::interfaces();
        return;
    }
    if has("--help") {
        print!("{USAGE}");
        return;
    }

    let dpi_set_at_runtime = win32::set_dpi_awareness();
    logging::init();
    install_panic_hook();
    // 매니페스트의 DPI 선언이 실제로 링크되었는지 로그로 확인한다(실패하면 런타임 폴백이다).
    logging::info(if dpi_set_at_runtime {
        "DPI: set PerMonitorV2 at runtime (no manifest)"
    } else {
        "DPI: using the PerMonitorV2 declaration from the manifest"
    });

    if !win32::acquire_single_instance() {
        // 이미 떠 있는 인스턴스가 있으면 그쪽에 설정 창을 부탁하고 조용히 끝난다.
        if ipc::request_settings() {
            logging::info("asked the running instance to open the settings window");
            return;
        }
        logging::info("already running, exiting");
        win32::inform(PRODUCT_NAME, &format!("{PRODUCT_NAME} is already running."));
        return;
    }

    let px = win32::tray_icon_size();
    logging::info(format!("tray icon size {px}px"));

    let app = match app::App::new(px) {
        Ok(app) => app,
        Err(error) => {
            logging::error(format!("startup failed: {error}"));
            win32::inform(
                PRODUCT_NAME,
                &format!("{PRODUCT_NAME} could not start.\n\n{error}"),
            );
            return;
        }
    };
    runtime::install(app);

    // 메뉴 클릭은 채널이 아니라 그 자리에서 처리한다. 설정 창의 모달 루프가 떠 있어도
    // 바로 반영되어야 한다(`runtime::menu_event`).
    tray_icon::menu::MenuEvent::set_event_handler(Some(|event: tray_icon::menu::MenuEvent| {
        runtime::menu_event(event.id.0)
    }));

    let interval = runtime::poll_interval();
    // 트레이 메뉴는 자체 메시지 루프를 돌리므로 숨은 창의 WndProc에서 타이머를 받아야
    // 메뉴가 열려 있어도 갱신이 계속된다.
    let timer = match timer::TimerWindow::new(interval.as_millis() as u32, runtime::tick) {
        Ok(timer) => timer,
        Err(error) => {
            logging::error(format!("could not create the timer: {error}"));
            win32::inform(
                PRODUCT_NAME,
                &format!("{PRODUCT_NAME} could not start.\n\n{error}"),
            );
            return;
        }
    };
    logging::info(format!(
        "poll interval {interval:?}, timer id {}",
        timer.id()
    ));

    // 두 번째 실행이 설정 창을 열 수 있게 창 하나를 띄워 둔다.
    let _listener = ipc::Listener::start();

    // `--settings`는 여기서 바로 열어도 된다(App을 빌리고 있지 않다).
    if wants_settings {
        settings::open();
    }

    unsafe {
        let mut message = MSG::default();
        loop {
            let result = GetMessageW(&mut message, None, 0, 0);
            if result.0 == 0 {
                // WM_QUIT
                break;
            }
            if result.0 == -1 {
                logging::error("GetMessageW failed");
                break;
            }

            let _ = TranslateMessage(&message);
            DispatchMessageW(&message);

            // 클릭 메시지가 도착하면 즉시 처리되도록 매 메시지마다 비운다.
            runtime::drain_events();

            // 설정 창은 **대여 밖에서만** 연다. 대여 중에 모달 루프에 들어가면 그동안
            // 타이머 틱이 전부 버려져 트레이가 멈춘다.
            if runtime::take_settings_request() {
                settings::open();
            }

            if runtime::should_exit() {
                break;
            }
        }
    }

    drop(timer);
    logging::info("exit");
}
