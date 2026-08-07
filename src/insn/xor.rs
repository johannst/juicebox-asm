// SPDX-License-Identifier: MIT
//
// Copyright (c) 2024, Johannes Stoelp <dev@memzero.de>

use super::Xor;
use crate::{Asm, Imm32, Reg32, Reg64};

impl Xor<Reg32, Reg32> for Asm {
    fn xor(&mut self, op1: Reg32, op2: Reg32) {
        self.encode_rr_mr(&[0x31], op1, op2);
    }
}

impl Xor<Reg64, Reg64> for Asm {
    fn xor(&mut self, op1: Reg64, op2: Reg64) {
        self.encode_rr_mr(&[0x31], op1, op2);
    }
}

impl Xor<Reg32, Imm32> for Asm {
    fn xor(&mut self, op1: Reg32, op2: Imm32) {
        self.encode_ri(0x81, 0x6, op1, op2);
    }
}
