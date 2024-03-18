use super::Cmovnz;
use crate::{Asm, Reg64};

impl Cmovnz<Reg64, Reg64> for Asm {
    fn cmovnz(&mut self, op1: Reg64, op2: Reg64) {
        self.encode_rr(&[0x0f, 0x45], op2, op1);
    }
}
