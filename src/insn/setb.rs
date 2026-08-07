// SPDX-License-Identifier: MIT
//
// Copyright (c) 2026, Johannes Stoelp <dev@memzero.de>

use super::Setb;
use crate::{Asm, Reg8};

impl Setb<Reg8> for Asm {
    fn setb(&mut self, op: Reg8) {
        self.encode_r(&[0xf, 0x92], 0, op);
    }
}
