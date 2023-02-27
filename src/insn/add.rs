use crate::prelude::*;

impl Add<Reg64, Reg64> for Asm {
    fn add(&mut self, op1: Reg64, op2: Reg64) {
        self.encode_rr(0x01, op1, op2);
    }
}
