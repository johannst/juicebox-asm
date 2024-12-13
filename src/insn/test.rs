use super::Test;
use crate::{Asm, Imm16, Mem16, Reg32, Reg64};

impl Test<Reg64, Reg64> for Asm {
    fn test(&mut self, op1: Reg64, op2: Reg64) {
        self.encode_rr(&[0x85], op1, op2);
    }
}

impl Test<Reg32, Reg32> for Asm {
    fn test(&mut self, op1: Reg32, op2: Reg32) {
        self.encode_rr(&[0x85], op1, op2);
    }
}

impl Test<Mem16, Imm16> for Asm {
    fn test(&mut self, op1: Mem16, op2: Imm16) {
        self.encode_mi(0xf7, 0, op1, op2);
    }
}
