// SPDX-License-Identifier: MIT
//
// Copyright (c) 2026, Johannes Stoelp <dev@memzero.de>

use super::Jge;
use crate::{Asm, Label};

impl Jge<&mut Label> for Asm {
    fn jge(&mut self, op1: &mut Label) {
        self.encode_jmp_label(&[0x0f, 0x8d], op1);
    }
}
