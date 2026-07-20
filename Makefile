BINARY  := tslay
BIN_DIR := target/release

VERSION := $(shell git describe --tags --always --dirty 2>/dev/null || echo "dev")

.PHONY: all build clean fmt fmt-check lint test install install-hooks

all: build

build:
	cargo build --release

install:
	cargo install --path .

fmt:
	cargo fmt

fmt-check:
	cargo fmt --check

lint:
	cargo clippy --all-targets -- -D warnings

test:
	cargo test

install-hooks:
	cp scripts/hooks/pre-commit .git/hooks/pre-commit
	chmod +x .git/hooks/pre-commit scripts/check.sh

clean:
	cargo clean
