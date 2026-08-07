// SPDX-License-Identifier: MIT
//
// Copyright (c) 2026, Johannes Stoelp <dev@memzero.de>

use super::Or;
use crate::{Asm, Imm32, Reg32};

impl Or<Reg32, Reg32> for Asm {
    fn or(&mut self, op1: Reg32, op2: Reg32) {
        self.encode_rr_mr(&[0x09], op1, op2);
    }
}

impl Or<Reg32, Imm32> for Asm {
    fn or(&mut self, op1: Reg32, op2: Imm32) {
        self.encode_ri(0x81, 0x1, op1, op2);
    }
}
