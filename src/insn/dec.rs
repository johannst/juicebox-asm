use super::Dec;
use crate::{Asm, Mem16, Mem32, Mem64, Mem8, Reg32, Reg64};

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

impl Dec<Mem8> for Asm {
    fn dec(&mut self, op1: Mem8) {
        self.encode_m(0xfe, 1, op1);
    }
}

impl Dec<Mem16> for Asm {
    fn dec(&mut self, op1: Mem16) {
        self.encode_m(0xff, 1, op1);
    }
}

impl Dec<Mem32> for Asm {
    fn dec(&mut self, op1: Mem32) {
        self.encode_m(0xff, 1, op1);
    }
}

impl Dec<Mem64> for Asm {
    fn dec(&mut self, op1: Mem64) {
        self.encode_m(0xff, 1, op1);
    }
}
