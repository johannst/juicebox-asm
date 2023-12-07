all: build build-examples check-fmt check-clippy check-tests check-examples

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
