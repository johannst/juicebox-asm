use super::Sub;
use crate::{Asm, Imm8, MemOp, Reg64};

impl Sub<Reg64, Reg64> for Asm {
    fn sub(&mut self, op1: Reg64, op2: Reg64) {
        self.encode_rr(&[0x29], op1, op2);
    }
}

impl Sub<MemOp, Imm8> for Asm {
    fn sub(&mut self, op1: MemOp, op2: Imm8) {
        self.encode_mi(0x83, 5, op1, op2);
    }
}
