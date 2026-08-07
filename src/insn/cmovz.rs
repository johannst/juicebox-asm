// SPDX-License-Identifier: MIT
//
// Copyright (c) 2024, Johannes Stoelp <dev@memzero.de>

use super::Cmovz;
use crate::{Asm, Reg64};

impl Cmovz<Reg64, Reg64> for Asm {
    fn cmovz(&mut self, op1: Reg64, op2: Reg64) {
        self.encode_rr_rm(&[0x0f, 0x44], op1, op2);
    }
}
