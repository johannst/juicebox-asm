pub mod prelude;

mod imm;
mod insn;
mod reg;

use imm::Imm;
pub use imm::{Imm16, Imm32, Imm64, Imm8};
use reg::Reg;
pub use reg::{Reg16, Reg32, Reg64, Reg8};

pub enum MemOp {
    Indirect(Reg64),
    IndirectDisp(Reg64, i32),
}

impl MemOp {
    const fn base(&self) -> Reg64 {
        match self {
            MemOp::Indirect(base) => *base,
            MemOp::IndirectDisp(base, ..) => *base,
        }
    }
}

/// Encode the `REX` byte.
const fn rex(w: bool, r: u8, x: u8, b: u8) -> u8 {
    let w = if w { 1 } else { 0 };
    let r = (r >> 3) & 1;
    let x = (x >> 3) & 1;
    let b = (b >> 3) & 1;
    0b0100_0000 | ((w & 1) << 3) | (r << 2) | (x << 1) | b
}

/// Encode the `ModR/M` byte.
const fn modrm(mod_: u8, reg: u8, rm: u8) -> u8 {
    ((mod_ & 0b11) << 6) | ((reg & 0b111) << 3) | (rm & 0b111)
}

pub struct Asm {
    buf: Vec<u8>,
}

impl Asm {
    pub fn new() -> Asm {
        let buf = Vec::with_capacity(1024);
        Asm { buf }
    }

    pub fn into_code(self) -> Vec<u8> {
        self.buf
    }

    fn emit(&mut self, bytes: &[u8]) {
        self.buf.extend_from_slice(bytes);
    }

    fn emit_optional(&mut self, bytes: &[Option<u8>]) {
        for byte in bytes.iter().filter_map(|&b| b) {
            self.buf.push(byte);
        }
    }

    fn emit_at(&mut self, pos: usize, bytes: &[u8]) {
        if let Some(buf) = self.buf.get_mut(pos..pos + bytes.len()) {
            buf.copy_from_slice(bytes);
        } else {
            unimplemented!();
        }
    }

    fn encode_rr<T: Reg>(&mut self, opc: u8, op1: T, op2: T)
    where
        Self: EncodeRR<T>,
    {
        // MR operand encoding.
        //   op1 -> modrm.rm
        //   op2 -> modrm.reg
        let modrm = modrm(
            0b11,      /* mod */
            op2.idx(), /* reg */
            op1.idx(), /* rm */
        );

        let prefix = <Self as EncodeRR<T>>::legacy_prefix();
        let rex = <Self as EncodeRR<T>>::rex(op1, op2);

        self.emit_optional(&[prefix, rex]);
        self.emit(&[opc, modrm]);
    }

    fn encode_oi<T: Reg, U: Imm>(&mut self, opc: u8, op1: T, op2: U)
    where
        Self: EncodeRI<T>,
    {
        let opc = opc + (op1.idx() & 0b111);
        let prefix = <Self as EncodeRI<T>>::legacy_prefix();
        let rex = <Self as EncodeRI<T>>::rex(op1);

        self.emit_optional(&[prefix, rex]);
        self.emit(&[opc]);
        self.emit(op2.bytes());
    }

    fn encode_ri<T: Reg, U: Imm>(&mut self, opc: u8, opc_ext: u8, op1: T, op2: U)
    where
        Self: EncodeRI<T>,
    {
        // MI operand encoding.
        //   op1 -> modrm.rm
        //   op2 -> modrm.reg
        let modrm = modrm(
            0b11,      /* mod */
            opc_ext,   /* reg */
            op1.idx(), /* rm */
        );

        let prefix = <Self as EncodeRI<T>>::legacy_prefix();
        let rex = <Self as EncodeRI<T>>::rex(op1);

        self.emit_optional(&[prefix, rex]);
        self.emit(&[opc, modrm]);
        self.emit(op2.bytes());
    }

    fn encode_mr<T: Reg>(&mut self, opc: u8, op1: MemOp, op2: T)
    where
        Self: EncodeMR<T>,
    {
        // MR operand encoding.
        //   op1 -> modrm.rm
        //   op2 -> modrm.reg
        let mode = match op1 {
            MemOp::Indirect(..) => {
                assert!(!op1.base().need_sib() && !op1.base().is_pc_rel());
                0b00
            }
            MemOp::IndirectDisp(..) => {
                assert!(!op1.base().need_sib());
                0b10
            }
        };

        let modrm = modrm(
            mode,             /* mode */
            op2.idx(),        /* reg */
            op1.base().idx(), /* rm */
        );
        let prefix = <Self as EncodeMR<T>>::legacy_prefix();
        let rex = <Self as EncodeMR<T>>::rex(&op1, op2);

        self.emit_optional(&[prefix, rex]);
        self.emit(&[opc, modrm]);
        if let MemOp::IndirectDisp(_, disp) = op1 {
            self.emit(&disp.to_ne_bytes());
        }
    }

    fn encode_rm<T: Reg>(&mut self, opc: u8, op1: T, op2: MemOp)
    where
        Self: EncodeMR<T>,
    {
        // RM operand encoding.
        //   op1 -> modrm.reg
        //   op2 -> modrm.rm
        self.encode_mr(opc, op2, op1);
    }
}

// -- Encoder helper.

trait EncodeRR<T: Reg> {
    fn legacy_prefix() -> Option<u8> {
        None
    }

    fn rex(op1: T, op2: T) -> Option<u8> {
        if op1.need_rex() || op2.need_rex() {
            Some(rex(op1.rexw(), op2.idx(), 0, op1.idx()))
        } else {
            None
        }
    }
}

impl EncodeRR<Reg8> for Asm {}
impl EncodeRR<Reg32> for Asm {}
impl EncodeRR<Reg16> for Asm {
    fn legacy_prefix() -> Option<u8> {
        Some(0x66)
    }
}
impl EncodeRR<Reg64> for Asm {}

trait EncodeRI<T: Reg> {
    fn legacy_prefix() -> Option<u8> {
        None
    }

    fn rex(op1: T) -> Option<u8> {
        if op1.need_rex() {
            Some(rex(op1.rexw(), 0, 0, op1.idx()))
        } else {
            None
        }
    }
}

impl EncodeRI<Reg8> for Asm {}
impl EncodeRI<Reg32> for Asm {}
impl EncodeRI<Reg16> for Asm {
    fn legacy_prefix() -> Option<u8> {
        Some(0x66)
    }
}
impl EncodeRI<Reg64> for Asm {}

trait EncodeMR<T: Reg> {
    fn legacy_prefix() -> Option<u8> {
        None
    }

    fn rex(op1: &MemOp, op2: T) -> Option<u8> {
        if op2.need_rex() || (op1.base().is_ext()) {
            Some(rex(op2.rexw(), op2.idx(), 0, op1.base().idx()))
        } else {
            None
        }
    }
}

impl EncodeMR<Reg8> for Asm {}
impl EncodeMR<Reg16> for Asm {
    fn legacy_prefix() -> Option<u8> {
        Some(0x66)
    }
}
impl EncodeMR<Reg32> for Asm {}
impl EncodeMR<Reg64> for Asm {}
