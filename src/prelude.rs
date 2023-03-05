//! Crate prelude, which can be used to import the most important types at once.

pub use crate::Asm;
pub use crate::MemOp;

pub use crate::imm::{Imm16, Imm32, Imm64, Imm8};
pub use crate::label::Label;
pub use crate::reg::{Reg16, Reg32, Reg64, Reg8};

pub use crate::insn::{Add, Dec, Jmp, Jnz, Jz, Mov, Test};
