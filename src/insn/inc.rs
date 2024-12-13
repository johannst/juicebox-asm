use super::Inc;
use crate::{Asm, Mem16, Mem32, Mem64, Mem8, Reg32, Reg64};

impl Inc<Reg64> for Asm {
    fn inc(&mut self, op1: Reg64) {
        self.encode_r(0xff, 0, op1);
    }
}

impl Inc<Reg32> for Asm {
    fn inc(&mut self, op1: Reg32) {
        self.encode_r(0xff, 0, op1);
    }
}

impl Inc<Mem8> for Asm {
    fn inc(&mut self, op1: Mem8) {
        self.encode_m(0xfe, 0, op1);
    }
}

impl Inc<Mem16> for Asm {
    fn inc(&mut self, op1: Mem16) {
        self.encode_m(0xff, 0, op1);
    }
}

impl Inc<Mem32> for Asm {
    fn inc(&mut self, op1: Mem32) {
        self.encode_m(0xff, 0, op1);
    }
}

impl Inc<Mem64> for Asm {
    fn inc(&mut self, op1: Mem64) {
        self.encode_m(0xff, 0, op1);
    }
}
