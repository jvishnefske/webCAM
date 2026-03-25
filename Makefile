.PHONY: build wasm lint test ts ci release serve clean help

WASM_TARGET = wasm32-unknown-unknown

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
