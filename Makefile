.PHONY: build wasm lint test ts ci release serve clean help verify \
       hil-firmware hil-stm32 hil-pi-zero hil-jlink-flash hil-verify all \
       dag-frontend combined-frontend embed-assets

WASM_TARGET = wasm32-unknown-unknown

# Crates excluded from host workspace builds:
# - Firmware binaries require embedded ARM targets and activate conflicting
#   critical-section features (embassy-rp → restore-state-u8 vs
#   cortex-m → restore-state-bool — mutually exclusive)
# - WASM frontends require wasm32-unknown-unknown target
# - Hardware-only crates (no_std USB/GPIO dispatchers, Pi Zero binary)
#   require physical hardware and cannot be tested on host
WORKSPACE_EXCLUDES = \
	--exclude board-support-pico \
	--exclude board-support-pico2 \
	--exclude board-support-stm32 \
	--exclude pico-bootloader \
	--exclude combined-frontend \
	--exclude hil-frontend \
	--exclude native-server \
	--exclude board-support-pi-zero \
	--exclude gs-usb-device \
	--exclude vprbrd-usb-gpio \
	--exclude usb-composite-dispatchers

all: hil-verify ci ## Run everything

help: ## Show this help
	@grep -E '^[a-zA-Z_-]+:.*##' $(MAKEFILE_LIST) | awk 'BEGIN {FS = ":.*## "}; {printf "  %-14s %s\n", $$1, $$2}'

build: ## Build native (for testing)
	cargo build --release

lint: ## Run clippy on all host crates (matches CI)
	cargo clippy --workspace $(WORKSPACE_EXCLUDES) --all-targets -- -D warnings

test: ## Run all library crate tests with coverage (llvm-cov)
	cargo llvm-cov --fail-under-functions 95 --workspace $(WORKSPACE_EXCLUDES)

wasm: ## Build WASM + JS bindings (requires wasm-pack)
	wasm-pack build --target web --out-dir www/pkg --release

ts: ## Build and test TypeScript frontend (matches CI)
	cd www && npm ci && npm run typecheck && npm run test && npm run build

verify: ## Build, test, lint, format-check (swiss-cheese gate)
	cargo build --workspace $(WORKSPACE_EXCLUDES) --all-targets
	cargo test --workspace $(WORKSPACE_EXCLUDES)
	cargo clippy --workspace $(WORKSPACE_EXCLUDES) --all-targets -- -D warnings
	cargo fmt --check

ci: lint test wasm ts ## Run full CI pipeline locally

release: wasm ts ## Package www/ for deployment (matches CI)
	cd www && zip -r ../rustcam.zip .
	@echo "Release artifact: rustcam.zip"

serve: wasm ts ## Build and serve locally (python)
	@echo "Serving at http://localhost:8080"
	@cd www && python3 -m http.server 8080

serve-native: wasm ## Build WASM + run native server with mock HAL
	cargo build -p native-server
	@echo "Starting native-server at http://localhost:3000"
	cargo run -p native-server -- --www-dir www --port 3000

clean: ## Remove build artifacts
	cargo clean
	rm -rf www/pkg www/dist www/node_modules rustcam.zip

# --- HIL targets ---

PICO_ELF    = hil/board-support-pico/target/thumbv6m-none-eabi/release/board-support-pico
PICO_BIN    = $(PICO_ELF).bin
JLINK       = JLinkExe
JLINK_SPEED = 4000

hil-jlink-flash: hil-firmware ## Flash Pico firmware via JLink
	arm-none-eabi-objcopy -O binary $(PICO_ELF) $(PICO_BIN)
	@printf 'r\nloadbin %s, 0x10000000\nr\ng\nq\n' "$(PICO_BIN)" > /tmp/jlink-pico.jlink
	$(JLINK) -device RP2040_M0_0 -if SWD -speed $(JLINK_SPEED) -autoconnect 1 -CommandFile /tmp/jlink-pico.jlink

hil-firmware: ## Build Pico firmware
	cargo build -p board-support-pico --target thumbv6m-none-eabi --release

combined-frontend: ## Build combined Leptos frontend (WASM) via Trunk
	cd hil/combined-frontend && trunk build --release

hil-pico2: combined-frontend dag-frontend ## Build Pico 2 firmware with DAG runtime
	EMBASSY_USB_MAX_INTERFACE_COUNT=16 EMBASSY_USB_MAX_HANDLER_COUNT=8 \
	cargo build -p board-support-pico2 --target thumbv8m.main-none-eabihf --release

hil-pico2-flash: hil-pico2 ## Flash Pico 2 via probe-rs
	probe-rs download --chip RP235x target/thumbv8m.main-none-eabihf/release/board-support-pico2
	probe-rs reset --chip RP235x

hil-stm32: ## Build STM32 firmware
	hil/scripts/build-stm32.sh firmware-out

hil-pi-zero: ## Build Pi Zero support crate
	cargo build -p board-support-pi-zero

hil-verify: hil-firmware ## Build all HIL firmware (host crates covered by ci)

# --- Embedded assets ---

dag-frontend: ## Build DAG editor JS bundle
	cd www && npx esbuild dag/dag-editor.ts --bundle --format=esm --target=es2022 --minify --external:../pkg/rustcam.js --outfile=dag/dag-editor.js

embed-assets: dag-frontend ## Gzip frontend assets and generate Rust source
	bash tools/embed-assets.sh

