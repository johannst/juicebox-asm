// SPDX-License-Identifier: MIT
//
// Copyright (c) 2023, Johannes Stoelp <dev@memzero.de>

use super::Call;
use crate::{Asm, Reg64};

impl Call<Reg64> for Asm {
    fn call(&mut self, op1: Reg64) {
        self.encode_r(&[0xff], 0x2, op1);
    }
}
