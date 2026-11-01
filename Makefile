# Checks run locally (no GitHub Actions in this repo; the product's own action.yml runs in users' repos).
# Rust comes from Homebrew rustup, which is not on the default PATH.
CARGO ?= $(shell command -v cargo 2>/dev/null || echo /opt/homebrew/opt/rustup/bin/cargo)
export PATH := $(dir $(CARGO)):$(PATH)

.PHONY: check test fmt boards gallery
check:
	$(CARGO) fmt --check
	$(CARGO) clippy --all-targets --all-features -- -D warnings
	$(CARGO) test --all-features

test:
	$(CARGO) test --all-features

fmt:
	$(CARGO) fmt

boards:
	$(CARGO) run -q --example board07

# README gallery: animated SVGs (and a GIF) from the persona fixtures, committed under docs/gallery.
gallery:
	$(CARGO) run -q --features gif --example gallery
	cp target/gallery/*.svg target/gallery/*.gif docs/gallery/
