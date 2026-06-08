# SPDX-License-Identifier: MIT
#
# Copyright (c) 2026, Johannes Stoelp <dev@memzero.de>

all: guest1 guest2

guest1: guest1.zig
	zig build-exe -O ReleaseFast --verbose-llvm-cpu-features \
		-target riscv32-freestanding-none -mcpu=generic_rv32 $<
	llvm-objdump -d $@

guest2: guest2.c
	@# Build for zalsrc required by musl, but extension not implemented.
	@# Crossing fingers we don't hit these insn during runtime :^)
	zig build-exe -O ReleaseFast --verbose-llvm-cpu-features \
		-target riscv32-linux-musl -mcpu=generic_rv32+zalrsc $< -lc
	llvm-objdump -d $@

run: guest
	qemu-riscv32 -strace $<
