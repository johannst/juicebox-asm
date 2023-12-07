use crate::Asm;

impl Asm {
    /// Emit a [`ret`](https://www.felixcloutier.com/x86/ret) instruction.
    pub fn ret(&mut self) {
        self.emit(&[0xc3]);
    }
}
