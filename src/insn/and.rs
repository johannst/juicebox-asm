// SPDX-License-Identifier: MIT
//
// Copyright (c) 2026, Johannes Stoelp <dev@memzero.de>

use super::And;
use crate::{Asm, Imm16, Imm32, Imm8, Reg16, Reg32, Reg8};

impl And<Reg8, Imm8> for Asm {
    fn and(&mut self, op1: Reg8, op2: Imm8) {
        self.encode_ri(0x80, 4, op1, op2);
    }
}

impl And<Reg16, Imm16> for Asm {
    fn and(&mut self, op1: Reg16, op2: Imm16) {
        self.encode_ri(0x81, 4, op1, op2);
    }
}

impl And<Reg32, Imm32> for Asm {
    fn and(&mut self, op1: Reg32, op2: Imm32) {
        self.encode_ri(0x81, 4, op1, op2);
    }
}

impl And<Reg32, Reg32> for Asm {
    fn and(&mut self, op1: Reg32, op2: Reg32) {
        self.encode_rr_mr(&[0x21], op1, op2);
    }
}
