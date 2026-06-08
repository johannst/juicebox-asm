// SPDX-License-Identifier: MIT
//
// Copyright (c) 2026, Johannes Stoelp <dev@memzero.de>

const SYS_WRITE = 64;
const SYS_EXIT = 93;

export fn _start() callconv(.naked) void {
    // Linux syscall ABI - man 2 syscall
    // - ecall a7=syscall -> {a0, a1}
    // - args: a0, a1, a2, a3, a4, a5

    const str = "moose-elk";
    _ = asm volatile (
        \\ecall
        : // No outputs.
        : [a7] "{a7}" (SYS_WRITE),
          [a0] "{a0}" (1),
          [a1] "{a1}" (str),
          [a2] "{a2}" (str.len),
    );

    _ = asm volatile (
        \\ecall
        : // No outputs.
        : [a7] "{a7}" (SYS_EXIT),
          [a0] "{a0}" (42),
    );
}
