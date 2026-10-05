.PHONY: build-desktop run-desktop setup-trunk run-web clean clippy help

.DEFAULT_GOAL := help


build-desktop: ## Build the desktop application
	cargo build --package event_camera_editor_ui

run-desktop: ## Run the desktop application
	cargo run --package event_camera_editor_ui

setup-trunk: ## Installs trunk on the host machine
	rustup target add wasm32-unknown-unknown
	cargo install --locked trunk

# detects if trunk is not already installed

ifeq ($(shell command -v trunk >/dev/null 2>&1; echo $$?),0)
	TRUNK_SETUP :=
else
	echo "Trunk is not installed, installing..."
	TRUNK_SETUP := setup-trunk
endif

run-web: $(TRUNK_SETUP) ## Serve the web application at http://localhost:8080
	trunk serve ui/src/index.html

clean: ## Remove build artifacts
	cargo clean

clippy: ## Run clippy linter
	cargo clippy -- -D warnings

help: ## Show this help message
	@echo "Usage: make [target]"
	@echo ""
	@echo "Targets:"
	@grep -E '^[a-zA-Z_-]+:.*## ' $(MAKEFILE_LIST) | sort | awk 'BEGIN {FS = ":.*## "}; {printf "  \033[36m%-20s\033[0m %s\n", $$1, $$2}'
