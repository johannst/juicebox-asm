use super::Sub;
use crate::{Asm, Imm8, Mem8, Reg64};

impl Sub<Reg64, Reg64> for Asm {
    fn sub(&mut self, op1: Reg64, op2: Reg64) {
        self.encode_rr(&[0x29], op1, op2);
    }
}

impl Sub<Mem8, Imm8> for Asm {
    fn sub(&mut self, op1: Mem8, op2: Imm8) {
        self.encode_mi(0x80, 5, op1, op2);
    }
}
