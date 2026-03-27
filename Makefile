.PHONY: docs-sync-loco docs-check-loco docs-sync-server-libs docs-check-server-libs
.PHONY: dev-start dev-stop dev-restart dev-logs dev-status
.PHONY: dev-admin dev-storefront dev-server
.PHONY: rust-test rust-doc-test rust-deps
.PHONY: install-tailwind
.PHONY: help

# ============================================================================
# Tailwind CSS tooling
# ============================================================================

# Build and install the vendored tailwind-rs CLI binary into ~/.cargo/bin.
# Must be run once before `trunk build` or `trunk serve` in apps/admin.
install-tailwind:
	cargo install --path crates/tailwind-rs

# ============================================================================
# Documentation targets
# ============================================================================

# Refresh metadata for the local Loco upstream docs snapshot.
docs-sync-loco:
	python3 scripts/loco_upstream_snapshot.py sync

# Validate that the upstream snapshot metadata exists and is fresh enough.
docs-check-loco:
	python3 scripts/loco_upstream_snapshot.py check

# Download fresh upstream docs snapshots for core server libraries.
docs-sync-server-libs:
	python3 scripts/server_library_docs_sync.py sync

# Validate local upstream docs snapshot for core server libraries.
docs-check-server-libs:
	python3 scripts/server_library_docs_sync.py check

# ============================================================================
# Development environment targets
# ============================================================================

# Start full development stack (server + 2 admins + 2 storefronts)
dev-start:
	@./scripts/dev-start.sh start

# Start only admin panels (Next.js + Leptos)
dev-admin:
	@./scripts/dev-start.sh start admin

# Start only storefronts (Next.js + Leptos)
dev-storefront:
	@./scripts/dev-start.sh start storefront

# Start only server + database
dev-server:
	@./scripts/dev-start.sh start server

# Stop all development services
dev-stop:
	@./scripts/dev-start.sh stop

# Restart all development services
dev-restart:
	@./scripts/dev-start.sh restart

# Follow logs for all services
dev-logs:
	@./scripts/dev-start.sh logs

# Show status of all services
dev-status:
	@./scripts/dev-start.sh status

# ============================================================================
# Rust quality targets
# ============================================================================

# Run the primary Rust workspace test suite with cargo-nextest.
rust-test:
	cargo nextest run --workspace --all-targets --all-features

# Keep doc-tests as a separate baseline alongside nextest.
rust-doc-test:
	cargo test --workspace --doc --all-features

# Check manifest dependency hygiene with cargo-machete.
rust-deps:
	cargo machete

# ============================================================================
# Help target
# ============================================================================

help:
	@echo "RusToK Makefile Commands"
	@echo ""
	@echo "Documentation:"
	@echo "  make docs-sync-loco        - Refresh Loco upstream docs snapshot"
	@echo "  make docs-check-loco       - Validate Loco upstream snapshot"
	@echo "  make docs-sync-server-libs - Download fresh server library docs"
	@echo "  make docs-check-server-libs - Validate server library docs"
	@echo ""
	@echo "Development:"
	@echo "  make dev-start             - Start full dev stack (all services)"
	@echo "  make dev-admin             - Start only admin panels"
	@echo "  make dev-storefront        - Start only storefronts"
	@echo "  make dev-server            - Start only server + database"
	@echo "  make dev-stop              - Stop all services"
	@echo "  make dev-restart           - Restart all services"
	@echo "  make dev-logs              - Follow logs"
	@echo "  make dev-status            - Show service status"
	@echo ""
	@echo "Rust quality:"
	@echo "  make rust-test             - Run Rust tests via cargo-nextest"
	@echo "  make rust-doc-test         - Run Rust doc-tests"
	@echo "  make rust-deps             - Check manifest dependency hygiene"
	@echo ""
	@echo "Quick Start:"
	@echo "  1. make dev-start          - Start everything"
	@echo "  2. Open http://localhost:3000 (Next.js Admin)"
	@echo "  3. Open http://localhost:3001 (Leptos Admin)"
	@echo "  4. Login: admin@local / admin12345"
	@echo ""
	@echo "Tailwind:"
	@echo "  make install-tailwind      - Build & install vendored tailwind-rs CLI"
	@echo ""
	@echo "For more info see: ./QUICKSTART.md"
