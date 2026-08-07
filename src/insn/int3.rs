// SPDX-License-Identifier: MIT
//
// Copyright (c) 2026, Johannes Stoelp <dev@memzero.de>

use crate::Asm;

impl Asm {
    /// Emit a [`in3`](https://www.felixcloutier.com/x86/intn:into:int3:int1) instruction.
    pub fn int3(&mut self) {
        self.emit(&[0xcc]);
    }
}
