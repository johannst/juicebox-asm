use crate::prelude::*;

impl Add<Reg64, Reg64> for Asm {
    fn add(&mut self, op1: Reg64, op2: Reg64) {
        self.encode_rr(0x01, op1, op2);
    }
}

impl Add<Reg32, Reg32> for Asm {
    fn add(&mut self, op1: Reg32, op2: Reg32) {
        self.encode_rr(0x01, op1, op2);
    }
}

impl Add<MemOp, Reg16> for Asm {
    fn add(&mut self, op1: MemOp, op2: Reg16) {
        self.encode_mr(0x01, op1, op2);
    }
}

impl Add<MemOp, Imm16> for Asm {
    fn add(&mut self, op1: MemOp, op2: Imm16) {
        self.encode_mi(0x81, 0, op1, op2);
    }
}
