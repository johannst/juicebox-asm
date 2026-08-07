// SPDX-License-Identifier: MIT
//
// Copyright (c) 2023, Johannes Stoelp <dev@memzero.de>

use super::Cmp;
use crate::{Asm, Imm16, Imm32, Imm8, Mem16, Mem8, Reg16, Reg32, Reg64, Reg8};

impl Cmp<Mem8, Imm8> for Asm {
    fn cmp(&mut self, op1: Mem8, op2: Imm8) {
        self.encode_mi(0x80, 0x7, op1, op2);
    }
}

impl Cmp<Mem16, Imm16> for Asm {
    fn cmp(&mut self, op1: Mem16, op2: Imm16) {
        self.encode_mi(0x81, 0x7, op1, op2);
    }
}

impl Cmp<Reg8, Imm8> for Asm {
    fn cmp(&mut self, op1: Reg8, op2: Imm8) {
        self.encode_ri(0x80, 0x7, op1, op2);
    }
}

impl Cmp<Reg16, Imm16> for Asm {
    fn cmp(&mut self, op1: Reg16, op2: Imm16) {
        self.encode_ri(0x81, 0x7, op1, op2);
    }
}

impl Cmp<Reg32, Imm32> for Asm {
    fn cmp(&mut self, op1: Reg32, op2: Imm32) {
        self.encode_ri(0x81, 0x7, op1, op2);
    }
}

impl Cmp<Reg32, Reg32> for Asm {
    fn cmp(&mut self, op1: Reg32, op2: Reg32) {
        self.encode_rr_mr(&[0x39], op1, op2);
    }
}

impl Cmp<Reg64, Reg64> for Asm {
    fn cmp(&mut self, op1: Reg64, op2: Reg64) {
        self.encode_rr_mr(&[0x39], op1, op2);
    }
}
