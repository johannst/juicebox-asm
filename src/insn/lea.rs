// SPDX-License-Identifier: MIT
//
// Copyright (c) 2026, Johannes Stoelp <dev@memzero.de>

use super::Lea;
use crate::{Asm, Mem16, Mem32, Mem64, Reg16, Reg32, Reg64};

impl Lea<Reg16, Mem16> for Asm {
    fn lea(&mut self, op1: Reg16, op2: Mem16) {
        self.encode_rm(&[0x8d], op1, op2);
    }
}

impl Lea<Reg32, Mem32> for Asm {
    fn lea(&mut self, op1: Reg32, op2: Mem32) {
        self.encode_rm(&[0x8d], op1, op2);
    }
}

impl Lea<Reg64, Mem64> for Asm {
    fn lea(&mut self, op1: Reg64, op2: Mem64) {
        self.encode_rm(&[0x8d], op1, op2);
    }
}
