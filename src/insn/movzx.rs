// SPDX-License-Identifier: MIT
//
// Copyright (c) 2026, Johannes Stoelp <dev@memzero.de>

use super::Movzx;
use crate::{Asm, Mem16, Mem8, Reg32, Reg8};

// -- MOVZX : reg reg

impl Movzx<Reg32, Reg8> for Asm {
    fn movzx(&mut self, op1: Reg32, op2: Reg8) {
        self.encode_rr_rm(&[0x0f, 0xb6], op1, op2);
    }
}

// -- MOVZX : reg mem

impl Movzx<Reg32, Mem8> for Asm {
    fn movzx(&mut self, op1: Reg32, op2: Mem8) {
        self.encode_rm(&[0x0f, 0xb6], op1, op2);
    }
}

impl Movzx<Reg32, Mem16> for Asm {
    fn movzx(&mut self, op1: Reg32, op2: Mem16) {
        self.encode_rm(&[0x0f, 0xb7], op1, op2);
    }
}
