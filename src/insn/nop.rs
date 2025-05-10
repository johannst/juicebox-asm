// SPDX-License-Identifier: MIT
//
// Copyright (c) 2023, Johannes Stoelp <dev@memzero.de>

use crate::Asm;

impl Asm {
    /// Emit a [`nop`](https://www.felixcloutier.com/x86/nop) instruction.
    pub fn nop(&mut self) {
        self.emit(&[0x90]);
    }
}
