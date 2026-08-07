// SPDX-License-Identifier: MIT
//
// Copyright (c) 2026, Johannes Stoelp <dev@memzero.de>

use super::Setl;
use crate::{Asm, Reg8};

impl Setl<Reg8> for Asm {
    fn setl(&mut self, op: Reg8) {
        self.encode_r(&[0xf, 0x9c], 0, op);
    }
}
