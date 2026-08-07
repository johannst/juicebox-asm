// SPDX-License-Identifier: MIT
//
// Copyright (c) 2026, Johannes Stoelp <dev@memzero.de>

use super::Jb;
use crate::{Asm, Label};

impl Jb<&mut Label> for Asm {
    fn jb(&mut self, op1: &mut Label) {
        self.encode_jmp_label(&[0x0f, 0x82], op1);
    }
}
