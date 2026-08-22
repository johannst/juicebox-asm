# SPDX-License-Identifier: MIT
#
# Copyright (c) 2026, Johannes Stoelp <dev@memzero.de>

GUESTS := guest1 guest2 guest3

all: $(GUESTS)

guest1: guest1.zig
	zig build-exe -O ReleaseFast --verbose-llvm-cpu-features \
		-target riscv32-freestanding-none -mcpu=generic_rv32 $<
	llvm-objdump -d $@

guest2: guest2.c
	zig build-exe -O ReleaseFast --verbose-llvm-cpu-features \
		-target riscv32-linux-musl -mcpu=generic_rv32+zalrsc $< -lc
	llvm-objdump -d $@

# Compile with --stack to overwrite the default stack size emitted in the
# GNU_STACK program header. The default size was quite big and I didn't want to
# give the VM too much memory.
#
# The stack size in the the GNU_STACK program header is used in case it is
# larger than the musl compiled in default, see
# https://git.musl-libc.org/cgit/musl/tree/src/env/__init_tls.c?h=f21a96538f78fa8e2040831b4209b35f2fb581da#n98
guest3: guest3.c
	zig build-exe -O ReleaseFast --verbose-llvm-cpu-features \
		-target riscv32-linux-musl -mcpu=generic_rv32+zalrsc $< -lc --stack 0x8000
	#llvm-objdump -d $@

fmt:
	clang-format --verbose -i --style="{BasedOnStyle: llvm, IndentWidth: 4}" *.c

run%: guest%
	qemu-riscv32 -strace $<

clean:
	rm -f $(GUESTS)
