use super::Dec;
use crate::{Asm, MemOp16, MemOp32, MemOp64, MemOp8, Reg32, Reg64};

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

impl Dec<MemOp8> for Asm {
    fn dec(&mut self, op1: MemOp8) {
        self.encode_m(0xfe, 1, op1);
    }
}

impl Dec<MemOp16> for Asm {
    fn dec(&mut self, op1: MemOp16) {
        self.encode_m(0xff, 1, op1);
    }
}

impl Dec<MemOp32> for Asm {
    fn dec(&mut self, op1: MemOp32) {
        self.encode_m(0xff, 1, op1);
    }
}

impl Dec<MemOp64> for Asm {
    fn dec(&mut self, op1: MemOp64) {
        self.encode_m(0xff, 1, op1);
    }
}
