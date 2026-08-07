// SPDX-License-Identifier: MIT
//
// Copyright (c) 2026, Johannes Stoelp <dev@memzero.de>

use super::Shr;
use crate::{imm::Imm, Asm, Imm8, Reg32, Reg8};

impl Shr<Reg32, Imm8> for Asm {
    fn shr(&mut self, op1: Reg32, op2: Imm8) {
        if op2.bytes()[0] == 1 {
            self.encode_r(&[0xd1], 0x5, op1);
        } else {
            self.encode_ri(0xc1, 0x5, op1, op2);
        }
    }
}

impl Shr<Reg32, Reg8> for Asm {
    fn shr(&mut self, op1: Reg32, op2: Reg8) {
        assert!(matches!(op2, Reg8::cl), "shl only takes cl");
        self.encode_r(&[0xd3], 0x5, op1);
    }
}
