use super::Xor;
use crate::{Asm, Reg64};

impl Xor<Reg64, Reg64> for Asm {
    fn xor(&mut self, op1: Reg64, op2: Reg64) {
        self.encode_rr(&[0x31], op1, op2);
    }
}
