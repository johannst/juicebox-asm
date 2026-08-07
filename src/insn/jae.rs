// SPDX-License-Identifier: MIT
//
// Copyright (c) 2026, Johannes Stoelp <dev@memzero.de>

use super::Jae;
use crate::{Asm, Label};

impl Jae<&mut Label> for Asm {
    fn jae(&mut self, op1: &mut Label) {
        self.encode_jmp_label(&[0x0f, 0x83], op1);
    }
}
