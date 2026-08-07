// SPDX-License-Identifier: MIT
//
// Copyright (c) 2024, Johannes Stoelp <dev@memzero.de>

use super::Cmovnz;
use crate::{Asm, Reg64};

impl Cmovnz<Reg64, Reg64> for Asm {
    fn cmovnz(&mut self, op1: Reg64, op2: Reg64) {
        self.encode_rr_rm(&[0x0f, 0x45], op1, op2);
    }
}
