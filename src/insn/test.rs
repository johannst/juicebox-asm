use crate::prelude::*;

impl Test<Reg64, Reg64> for Asm {
    fn test(&mut self, op1: Reg64, op2: Reg64) {
        self.encode_rr(0x85, op1, op2);
    }
}
