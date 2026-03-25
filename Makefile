.PHONY: build wasm lint test ts ci release serve clean help \
       hil-analyze hil-test hil-firmware hil-stm32 hil-pi-zero hil-verify all

WASM_TARGET = wasm32-unknown-unknown
HIL_HOST_PKGS = -p i2c-hil-sim -p i2c-hil-devices -p hil-backplane -p board-config-common -p hil-frontend

help: ## Show this help
	@grep -E '^[a-zA-Z_-]+:.*##' $(MAKEFILE_LIST) | awk 'BEGIN {FS = ":.*## "}; {printf "  %-14s %s\n", $$1, $$2}'

build: ## Build native (for testing)
	cargo build --release

lint: ## Run clippy (matches CI)
	cargo clippy --all-targets -- -D warnings

test: ## Run all workspace tests (matches CI)
	cargo test --all

wasm: ## Build WASM + JS bindings (requires wasm-pack)
	wasm-pack build --target web --out-dir www/pkg --release

ts: ## Build TypeScript frontend (matches CI)
	cd www && npm ci && npm run typecheck && npm run build

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

hil-analyze: ## Run clippy on host-compatible HIL crates
	cargo clippy $(HIL_HOST_PKGS) -- -D warnings

hil-test: ## Run tests on host-compatible HIL crates
	cargo test $(HIL_HOST_PKGS)
	cargo test -p board-support-pi-zero

hil-firmware: ## Build Pico firmware
	cargo build-pico

hil-stm32: ## Build STM32 firmware
	hil/scripts/build-stm32.sh firmware-out

hil-pi-zero: ## Build Pi Zero support crate
	cargo build -p board-support-pi-zero

hil-verify: hil-analyze hil-test hil-firmware ## Full HIL verification

all: ci hil-verify ## Run everything
