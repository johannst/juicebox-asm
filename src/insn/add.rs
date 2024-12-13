use super::Add;
use crate::{Asm, Imm16, Imm8, Mem16, Mem32, Mem64, Mem8, Reg16, Reg32, Reg64};

impl Add<Reg32, Reg32> for Asm {
    fn add(&mut self, op1: Reg32, op2: Reg32) {
        self.encode_rr(&[0x01], op1, op2);
    }
}

impl Add<Reg64, Reg64> for Asm {
    fn add(&mut self, op1: Reg64, op2: Reg64) {
        self.encode_rr(&[0x01], op1, op2);
    }
}

impl Add<Mem16, Reg16> for Asm {
    fn add(&mut self, op1: Mem16, op2: Reg16) {
        self.encode_mr(0x01, op1, op2);
    }
}

impl Add<Mem64, Reg64> for Asm {
    fn add(&mut self, op1: Mem64, op2: Reg64) {
        self.encode_mr(0x01, op1, op2);
    }
}

impl Add<Reg64, Mem64> for Asm {
    fn add(&mut self, op1: Reg64, op2: Mem64) {
        self.encode_rm(0x03, op1, op2);
    }
}

impl Add<Mem8, Imm8> for Asm {
    fn add(&mut self, op1: Mem8, op2: Imm8) {
        self.encode_mi(0x80, 0, op1, op2);
    }
}

impl Add<Mem16, Imm8> for Asm {
    fn add(&mut self, op1: Mem16, op2: Imm8) {
        self.encode_mi(0x83, 0, op1, op2);
    }
}

impl Add<Mem32, Imm8> for Asm {
    fn add(&mut self, op1: Mem32, op2: Imm8) {
        self.encode_mi(0x83, 0, op1, op2);
    }
}

impl Add<Mem64, Imm8> for Asm {
    fn add(&mut self, op1: Mem64, op2: Imm8) {
        self.encode_mi(0x83, 0, op1, op2);
    }
}

impl Add<Mem16, Imm16> for Asm {
    fn add(&mut self, op1: Mem16, op2: Imm16) {
        self.encode_mi(0x81, 0, op1, op2);
    }
}
