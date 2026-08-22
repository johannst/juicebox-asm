# SPDX-License-Identifier: MIT
#
# Copyright (c) 2024, Johannes Stoelp <dev@memzero.de>

default: debug release

debug:
	$(MAKE) all

release:
	$(MAKE) all CARGO_FLAGS=--release

all: build build-examples check-fmt check-clippy check-tests check-examples check-readme run-examples

build:
	cargo build $(CARGO_FLAGS)

build-examples: build-guests
	cargo build $(CARGO_FLAGS) --examples

build-guests:
	make -C ../examples/rv32i-guest

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

run-examples: build-guests
	cargo run $(CARGO_FLAGS) --example fib
	cargo run $(CARGO_FLAGS) --example add
	cargo run $(CARGO_FLAGS) --example tiny_vm
	cargo run $(CARGO_FLAGS) --example tiny_vm jit
	cargo run $(CARGO_FLAGS) --example bf
	cargo run $(CARGO_FLAGS) --example rv32i guest1 | tee ../target/rv32i.txt
	grep "^moose-elk$$" ../target/rv32i.txt
	cargo run $(CARGO_FLAGS) --example rv32i guest2 | tee ../target/rv32i.txt
	grep "^hello C, argc=3$$" ../target/rv32i.txt
	grep "^argv\[0\]=guest2$$" ../target/rv32i.txt
	grep "^argv\[1\]=moose$$" ../target/rv32i.txt
	grep "^argv\[2\]=elk$$" ../target/rv32i.txt
	cargo run $(CARGO_FLAGS) --example rv32i guest3 | tee ../target/rv32i.txt
	grep "^thread1: th_local_val=4242$$" ../target/rv32i.txt
	grep "^thread2 arg=thread2 some arg$$" ../target/rv32i.txt
	grep "^main: pthread_join() -> 0x1337$$" ../target/rv32i.txt
