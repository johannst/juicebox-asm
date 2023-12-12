use super::Pop;
use crate::{Asm, Reg16, Reg64};

impl Pop<Reg64> for Asm {
    fn pop(&mut self, op1: Reg64) {
        self.encode_r(0x8f, 0x0, op1);
    }
}

impl Pop<Reg16> for Asm {
    fn pop(&mut self, op1: Reg16) {
        self.encode_r(0x8f, 0x0, op1);
    }
}
