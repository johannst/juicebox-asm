use crate::prelude::*;

impl Cmp<MemOp, Imm16> for Asm {
    fn cmp(&mut self, op1: MemOp, op2: Imm16) {
        self.encode_mi(0x81, 0x7, op1, op2);
    }
}
