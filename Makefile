# tray-monitor 개발 명령
#
# make validate : 커밋 전 필수 검사 (CI 의 Quality Gate 와 같은 검사)
# make dump     : 아이콘 렌더 결과를 덤프하고 비교 시트를 만든다

.PHONY: validate fmt clippy test run build install probe interfaces dump clean

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
	python tools/icon_sheet.py out/sheet-dark.bmp 16 dark_px16 dark_px24 dark_px32
	python tools/icon_sheet.py out/sheet-light.bmp 16 light_px16 light_px24 light_px32

clean:
	cargo clean
	rm -rf out shots
