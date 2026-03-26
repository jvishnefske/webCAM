.PHONY: build wasm lint test ts ci release serve clean help \
       hil-firmware hil-stm32 hil-pi-zero hil-jlink-flash hil-verify all \
       dag-frontend embed-assets

WASM_TARGET = wasm32-unknown-unknown

help: ## Show this help
	@grep -E '^[a-zA-Z_-]+:.*##' $(MAKEFILE_LIST) | awk 'BEGIN {FS = ":.*## "}; {printf "  %-14s %s\n", $$1, $$2}'

build: ## Build native (for testing)
	cargo build --release

lint: ## Run clippy (matches CI)
	cargo clippy --all-targets -- -D warnings

test: ## Run all default-member tests (matches CI)
	cargo test

wasm: ## Build WASM + JS bindings (requires wasm-pack)
	wasm-pack build --target web --out-dir www/pkg --release

ts: ## Build and test TypeScript frontend (matches CI)
	cd www && npm ci && npm run typecheck && npm run test && npm run build

ci: lint test wasm ts ## Run full CI pipeline locally

release: wasm ts ## Package www/ for deployment (matches CI)
	cd www && zip -r ../rustcam.zip .
	@echo "Release artifact: rustcam.zip"

serve: wasm ts ## Build and serve locally
	@echo "Serving at http://localhost:8080"
	@cd www && python3 -m http.server 8080

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
	cargo build-pico

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

all: ci hil-verify ## Run everything
