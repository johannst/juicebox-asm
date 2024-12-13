use super::Cmp;
use crate::{Asm, Imm16, Imm8, Mem16, Mem8};

impl Cmp<Mem8, Imm8> for Asm {
    fn cmp(&mut self, op1: Mem8, op2: Imm8) {
        self.encode_mi(0x80, 0x7, op1, op2);
    }
}

impl Cmp<Mem16, Imm16> for Asm {
    fn cmp(&mut self, op1: Mem16, op2: Imm16) {
        self.encode_mi(0x81, 0x7, op1, op2);
    }
}
