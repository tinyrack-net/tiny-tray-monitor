# tray-monitor 개발 명령
#
# make validate : 커밋 전 필수 검사 (CI 의 Quality Gate 와 같은 검사)
# make dump     : 아이콘 렌더 결과를 덤프하고 비교 시트를 만든다

.PHONY: validate fmt clippy test run build install probe interfaces dump glyphs assets installer msix package clean

validate: fmt clippy test

fmt:
	cargo fmt --all -- --check

clippy:
	cargo clippy --all-targets -- -D warnings

test:
	cargo test --all-targets

run:
	cargo run --release

build:
	cargo build --release

probe:
	cargo run --release -- --probe

interfaces:
	cargo run --release -- --interfaces

# 아이콘 레이아웃을 바꿨다면 시트를 눈으로 확인한다.
dump:
	cargo run --release -- --dump
	python tools/icon_sheet.py out/sheet-dark.bmp 16 dark_px16 dark_px20 dark_px24 dark_px32
	python tools/icon_sheet.py out/sheet-light.bmp 16 light_px16 light_px20 light_px24 light_px32

# 글리프 표를 다시 만든다. 결과는 커밋한다.
glyphs:
	python tools/pixelfont.py

glyphs-check:
	python tools/pixelfont.py --check

# 아이콘/타일 자산을 다시 만든다. 결과는 커밋한다.
assets:
	python tools/make_assets.py

# installer와 MSIX는 미리 빌드된 exe에서 버전을 읽는다.
installer: build
	pwsh -NoProfile -File packaging/windows/build-installer.ps1 -Exe target/release/tray-monitor.exe -OutputDir dist

msix: build
	pwsh -NoProfile -File packaging/windows/build-msix.ps1 -Exe target/release/tray-monitor.exe -OutputDir dist

package: installer msix

clean:
	cargo clean
	rm -rf out shots dist
