use super::Jmp;
use crate::{Asm, Label};

impl Jmp<&mut Label> for Asm {
    fn jmp(&mut self, op1: &mut Label) {
        self.encode_jmp_label(&[0xe9], op1);
    }
}
