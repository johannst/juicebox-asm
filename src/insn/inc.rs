use super::Inc;
use crate::{Asm, MemOp16, MemOp32, MemOp64, MemOp8, Reg32, Reg64};

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

impl Inc<MemOp8> for Asm {
    fn inc(&mut self, op1: MemOp8) {
        self.encode_m(0xfe, 0, op1);
    }
}

impl Inc<MemOp16> for Asm {
    fn inc(&mut self, op1: MemOp16) {
        self.encode_m(0xff, 0, op1);
    }
}

impl Inc<MemOp32> for Asm {
    fn inc(&mut self, op1: MemOp32) {
        self.encode_m(0xff, 0, op1);
    }
}

impl Inc<MemOp64> for Asm {
    fn inc(&mut self, op1: MemOp64) {
        self.encode_m(0xff, 0, op1);
    }
}
