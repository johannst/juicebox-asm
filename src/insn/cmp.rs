use super::Cmp;
use crate::{Asm, Imm16, Imm8, MemOp};

impl Cmp<MemOp, Imm8> for Asm {
    fn cmp(&mut self, op1: MemOp, op2: Imm8) {
        self.encode_mi(0x80, 0x7, op1, op2);
    }
}

impl Cmp<MemOp, Imm16> for Asm {
    fn cmp(&mut self, op1: MemOp, op2: Imm16) {
        self.encode_mi(0x81, 0x7, op1, op2);
    }
}
