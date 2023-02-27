use crate::prelude::*;

impl Dec<Reg64> for Asm {
    fn dec(&mut self, op1: Reg64) {
        self.encode_r(0xff, 1, op1);
    }
}

impl Dec<Reg32> for Asm {
    fn dec(&mut self, op1: Reg32) {
        self.encode_r(0xff, 1, op1);
    }
}
