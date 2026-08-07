// SPDX-License-Identifier: MIT
//
// Copyright (c) 2026, Johannes Stoelp <dev@memzero.de>

use super::Movsx;
use crate::{Asm, Mem16, Mem32, Mem8, Reg32, Reg64};

// -- MOVSX : reg mem

impl Movsx<Reg32, Mem8> for Asm {
    fn movsx(&mut self, op1: Reg32, op2: Mem8) {
        self.encode_rm(&[0x0f, 0xbe], op1, op2);
    }
}

impl Movsx<Reg32, Mem16> for Asm {
    fn movsx(&mut self, op1: Reg32, op2: Mem16) {
        self.encode_rm(&[0x0f, 0xbf], op1, op2);
    }
}

impl Movsx<Reg64, Mem32> for Asm {
    fn movsx(&mut self, op1: Reg64, op2: Mem32) {
        self.encode_rm(&[0x63], op1, op2);
    }
}
