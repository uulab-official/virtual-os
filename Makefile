.PHONY: build test clippy fmt sign run-daemon clean

build:
	cargo build --workspace

test:
	cargo test --workspace

clippy:
	cargo clippy --workspace --all-targets -- -D warnings

fmt:
	cargo fmt --all

sign:
	./scripts/sign-dev.sh debug

run-daemon:
	./scripts/run-dev.sh daemon start --foreground

clean:
	cargo clean
