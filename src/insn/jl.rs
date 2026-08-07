// SPDX-License-Identifier: MIT
//
// Copyright (c) 2026, Johannes Stoelp <dev@memzero.de>

use super::Jl;
use crate::{Asm, Label};

impl Jl<&mut Label> for Asm {
    fn jl(&mut self, op1: &mut Label) {
        self.encode_jmp_label(&[0x0f, 0x8c], op1);
    }
}
