default: debug release

debug:
	$(MAKE) all

release:
	$(MAKE) all CARGO_FLAGS=--release

all: build build-examples check-fmt check-clippy check-tests check-examples check-readme run-examples

build:
	cargo build $(CARGO_FLAGS)

build-examples:
	cargo build $(CARGO_FLAGS) --examples

check-fmt:
	cargo fmt --check

check-clippy:
	cargo clippy -- -Dwarnings -Aclippy::new_without_default

check-tests:
	cargo test $(CARGO_FLAGS)

check-examples:
	cargo test $(CARGO_FLAGS) --examples

check-readme:
	awk '/^```rust$$/,/^```$$/ { if (!($$1 ~ "^```")) { print } }' ../README.md > ../examples/readme.rs
	cargo run $(CARGO_FLAGS) --example readme
	rm -f ../examples/readme.rs

run-examples:
	cargo run $(CARGO_FLAGS) --example fib
	cargo run $(CARGO_FLAGS) --example add
	cargo run $(CARGO_FLAGS) --example tiny_vm
	cargo run $(CARGO_FLAGS) --example tiny_vm jit
	cargo run $(CARGO_FLAGS) --example bf
