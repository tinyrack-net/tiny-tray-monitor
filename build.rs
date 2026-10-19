//! 리소스 컴파일. `resources/app.rc`를 컴파일해 exe에 링크한다.
//!
//! 리소스에는 파일 속성에 보이는 `VERSIONINFO`와 매니페스트(비주얼 스타일 · PerMonitorV2 DPI)가
//! 들어간다. 버전 숫자를 `.rc`에 손으로 적으면 `Cargo.toml`과 반드시 어긋나므로 여기서
//! `version_defines.rc`를 만들어 `.rc`가 include하게 한다.
//!
//! `rc.exe`는 VS 개발자 프롬프트 밖에서 기본 include 경로가 없어 `windows.h`를 찾지 못한다.
//! `embed-resource`가 Windows Kits의 `Include\<version>` 디렉터리들로 `%INCLUDE%`를 채워 주므로
//! 별도 설정 없이 동작한다(실측, `docs/SPIKE.md` §5).

use std::env;
use std::fs;
use std::path::PathBuf;

fn main() {
    for path in ["resources/app.rc", "resources/app.manifest", "build.rs"] {
        println!("cargo:rerun-if-changed={path}");
    }

    write_version_defines();

    // 매니페스트가 빠지면 비주얼 스타일과 DPI가 조용히 깨지므로 실패를 넘기지 않는다.
    embed_resource::compile("resources/app.rc", embed_resource::NONE)
        .manifest_required()
        .expect("리소스를 컴파일해 링크할 수 없다");
}

/// `Cargo.toml`의 버전을 `.rc`가 쓸 수 있는 형태로 옮겨 적는다.
fn write_version_defines() {
    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR"));
    let version = env::var("CARGO_PKG_VERSION").expect("CARGO_PKG_VERSION");
    let [major, minor, patch, build] = version_quad(&version);
    let defines = format!(
        "\
// build.rs가 만든다. 손으로 고치지 않는다.
#define APP_VERSION_MAJOR {major}
#define APP_VERSION_MINOR {minor}
#define APP_VERSION_PATCH {patch}
#define APP_VERSION_BUILD {build}
#define APP_VERSION_STRING \"{version}\"
"
    );
    fs::write(out_dir.join("version_defines.rc"), defines)
        .expect("version_defines.rc를 쓸 수 없다");
}

/// `0.2.0` → `[0, 2, 0, 0]`. VERSIONINFO는 숫자 네 칸만 받는다.
fn version_quad(version: &str) -> [u32; 4] {
    let mut quad = [0; 4];
    for (slot, part) in quad.iter_mut().zip(version.split(['.', '-', '+'])) {
        *slot = part.parse().unwrap_or(0);
    }
    quad
}
