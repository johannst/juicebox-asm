use super::Inc;
use crate::{Asm, Reg32, Reg64};

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
