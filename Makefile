.PHONY: build wasm test clean release serve help verify ts

WASM_TARGET = wasm32-unknown-unknown

help: ## Show this help
	@grep -E '^[a-zA-Z_-]+:.*##' $(MAKEFILE_LIST) | awk 'BEGIN {FS = ":.*## "}; {printf "  %-14s %s\n", $$1, $$2}'

build: ## Build native (for testing)
	cargo build --release

test: ## Run unit tests
	cargo test

wasm: ## Build WASM + JS bindings (requires wasm-pack)
	wasm-pack build --target web --out-dir www/pkg --release

ts: ## Build TypeScript frontend
	cd www && npm install --silent && npm run build

release: wasm ts ## Package www/ for GitHub release
	@mkdir -p dist
	cp -r www dist/rustcam
	cd dist && zip -r rustcam.zip rustcam/
	@echo "Release artifact: dist/rustcam.zip"

serve: wasm ts ## Build and serve locally
	@echo "Serving at http://localhost:8080"
	@cd www && python3 -m http.server 8080

clean: ## Remove build artifacts
	cargo clean
	rm -rf www/pkg www/dist www/node_modules dist

HOST_TARGET := $(shell rustc -vV | grep host | awk '{print $$2}')

verify: ## Run all verification checks (override parent embedded target)
	cargo fmt --check
	cargo clippy --target $(HOST_TARGET) --all-targets -- -D warnings
	cargo test --target $(HOST_TARGET)
	cd www && npm install --silent && npm run typecheck && npm run build
	@command -v wasm-pack >/dev/null && wasm-pack build --target web --out-dir www/pkg --release || echo "Skipping WASM build (wasm-pack not installed)"
