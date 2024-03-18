use super::Cmovz;
use crate::{Asm, Reg64};

impl Cmovz<Reg64, Reg64> for Asm {
    fn cmovz(&mut self, op1: Reg64, op2: Reg64) {
        self.encode_rr(&[0x0f, 0x44], op2, op1);
    }
}
