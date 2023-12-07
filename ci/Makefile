all: build build-examples check-fmt check-clippy check-tests check-examples run-examples

build:
	cargo build

build-examples:
	cargo build --examples

check-fmt:
	cargo fmt --check

check-clippy:
	cargo clippy -- -Dwarnings -Aclippy::new_without_default

check-tests:
	cargo test

check-examples:
	cargo test --examples

run-examples:
	cargo run --example fib
	cargo run --example add
	cargo run --example tiny_vm
	cargo run --example tiny_vm jit
