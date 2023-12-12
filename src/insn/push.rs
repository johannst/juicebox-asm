use super::Push;
use crate::{Asm, Reg16, Reg64};

impl Push<Reg64> for Asm {
    fn push(&mut self, op1: Reg64) {
        self.encode_r(0xff, 0x6, op1);
    }
}

impl Push<Reg16> for Asm {
    fn push(&mut self, op1: Reg16) {
        self.encode_r(0xff, 0x6, op1);
    }
}
