//! The `x64` jit assembler.

use crate::*;
use imm::Imm;
use reg::Reg;

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

/// Encode the `SIB` byte.
const fn sib(scale: u8, index: u8, base: u8) -> u8 {
    ((scale & 0b11) << 6) | ((index & 0b111) << 3) | (base & 0b111)
}

/// `x64` jit assembler.
pub struct Asm {
    buf: Vec<u8>,
}

impl Asm {
    /// Create a new `x64` jit assembler.
    pub fn new() -> Asm {
        // Some random default capacity.
        let buf = Vec::with_capacity(1024);
        Asm { buf }
    }

    /// Consume the assembler and get the emitted code.
    pub fn into_code(self) -> Vec<u8> {
        self.buf
    }

    /// Emit a slice of bytes.
    pub(crate) fn emit(&mut self, bytes: &[u8]) {
        self.buf.extend_from_slice(bytes);
    }

    /// Emit a slice of optional bytes.
    fn emit_optional(&mut self, bytes: &[Option<u8>]) {
        for byte in bytes.iter().filter_map(|&b| b) {
            self.buf.push(byte);
        }
    }

    /// Emit a slice of bytes at `pos`.
    ///
    /// # Panics
    ///
    /// Panics if [pos..pos+len] indexes out of bound of the underlying code buffer.
    fn emit_at(&mut self, pos: usize, bytes: &[u8]) {
        if let Some(buf) = self.buf.get_mut(pos..pos + bytes.len()) {
            buf.copy_from_slice(bytes);
        } else {
            unimplemented!();
        }
    }

    /// Bind the [Label] to the current location.
    pub fn bind(&mut self, label: &mut Label) {
        // Bind the label to the current offset.
        label.bind(self.buf.len());

        // Resolve any pending relocations for the label.
        self.resolve(label);
    }

    /// If the [Label] is bound, patch any pending relocation.
    fn resolve(&mut self, label: &mut Label) {
        if let Some(loc) = label.location() {
            // For now we only support disp32 as label location.
            let loc = i32::try_from(loc).expect("Label location did not fit into i32.");

            // Resolve any pending relocations for the label.
            for off in label.offsets_mut().drain() {
                // Displacement is relative to the next instruction following the jump.
                // We record the offset to patch at the first byte of the disp32 therefore we need
                // to account for that in the disp computation.
                let disp32 = loc - i32::try_from(off).expect("Label offset did not fit into i32") - 4 /* account for the disp32 */;

                // Patch the relocation with the disp32.
                self.emit_at(off, &disp32.to_ne_bytes());
            }
        }
    }

    // -- Encode utilities.

    /// Encode an register-register instruction.
    pub(crate) fn encode_rr<T: Reg>(&mut self, opc: &[u8], op1: T, op2: T)
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
        self.emit(opc);
        self.emit(&[modrm]);
    }

    /// Encode an offset-immediate instruction.
    /// Register idx is encoded in the opcode.
    pub(crate) fn encode_oi<T: Reg, U: Imm>(&mut self, opc: u8, op1: T, op2: U)
    where
        Self: EncodeR<T>,
    {
        let opc = opc + (op1.idx() & 0b111);
        let prefix = <Self as EncodeR<T>>::legacy_prefix();
        let rex = <Self as EncodeR<T>>::rex(op1);

        self.emit_optional(&[prefix, rex]);
        self.emit(&[opc]);
        self.emit(op2.bytes());
    }

    /// Encode a register instruction.
    pub(crate) fn encode_r<T: Reg>(&mut self, opc: u8, opc_ext: u8, op1: T)
    where
        Self: EncodeR<T>,
    {
        // M operand encoding.
        //   op1           -> modrm.rm
        //   opc extension -> modrm.reg
        let modrm = modrm(
            0b11,      /* mod */
            opc_ext,   /* reg */
            op1.idx(), /* rm */
        );

        let prefix = <Self as EncodeR<T>>::legacy_prefix();
        let rex = <Self as EncodeR<T>>::rex(op1);

        self.emit_optional(&[prefix, rex]);
        self.emit(&[opc, modrm]);
    }

    /// Encode a memory-immediate instruction.
    pub(crate) fn encode_mi<T: Imm>(&mut self, opc: u8, opc_ext: u8, op1: MemOp, op2: T)
    where
        Self: EncodeMI<T>,
    {
        // MI operand encoding.
        //   op1 -> modrm.rm
        //   op2 -> imm
        let (mode, rm) = match op1 {
            MemOp::Indirect(..) => {
                assert!(!op1.base().need_sib() && !op1.base().is_pc_rel());
                (0b00, op1.base().idx())
            }
            MemOp::IndirectDisp(..) => {
                assert!(!op1.base().need_sib());
                (0b10, op1.base().idx())
            }
            MemOp::IndirectBaseIndex(..) => {
                assert!(!op1.base().is_pc_rel());
                // Using rsp as index register is interpreted as just base w/o offset.
                //   https://wiki.osdev.org/X86-64_Instruction_Encoding#32.2F64-bit_addressing_2
                // Disallow this case, as guard for the user.
                assert!(!matches!(op1.index(), Reg64::rsp));
                (0b00, 0b100)
            }
        };

        let modrm = modrm(
            mode,    /* mode */
            opc_ext, /* reg */
            rm,      /* rm */
        );

        let prefix = <Self as EncodeMI<T>>::legacy_prefix();
        let rex = <Self as EncodeMI<T>>::rex(&op1);

        self.emit_optional(&[prefix, rex]);
        self.emit(&[opc, modrm]);
        match op1 {
            MemOp::Indirect(..) => {}
            MemOp::IndirectDisp(_, disp) => self.emit(&disp.to_ne_bytes()),
            MemOp::IndirectBaseIndex(base, index) => self.emit(&[sib(0, index.idx(), base.idx())]),
        }
        self.emit(op2.bytes());
    }

    /// Encode a memory-register instruction.
    pub(crate) fn encode_mr<T: Reg>(&mut self, opc: u8, op1: MemOp, op2: T)
    where
        Self: EncodeMR<T>,
    {
        // MR operand encoding.
        //   op1 -> modrm.rm
        //   op2 -> modrm.reg
        let (mode, rm) = match op1 {
            MemOp::Indirect(..) => {
                assert!(!op1.base().need_sib() && !op1.base().is_pc_rel());
                (0b00, op1.base().idx())
            }
            MemOp::IndirectDisp(..) => {
                assert!(!op1.base().need_sib());
                (0b10, op1.base().idx())
            }
            MemOp::IndirectBaseIndex(..) => {
                assert!(!op1.base().is_pc_rel());
                // Using rsp as index register is interpreted as just base w/o offset.
                //   https://wiki.osdev.org/X86-64_Instruction_Encoding#32.2F64-bit_addressing_2
                // Disallow this case, as guard for the user.
                assert!(!matches!(op1.index(), Reg64::rsp));
                (0b00, 0b100)
            }
        };

        let modrm = modrm(
            mode,      /* mode */
            op2.idx(), /* reg */
            rm,        /* rm */
        );

        let prefix = <Self as EncodeMR<T>>::legacy_prefix();
        let rex = <Self as EncodeMR<T>>::rex(&op1, op2);

        self.emit_optional(&[prefix, rex]);
        self.emit(&[opc, modrm]);
        match op1 {
            MemOp::Indirect(..) => {}
            MemOp::IndirectDisp(_, disp) => self.emit(&disp.to_ne_bytes()),
            MemOp::IndirectBaseIndex(base, index) => self.emit(&[sib(0, index.idx(), base.idx())]),
        }
    }

    /// Encode a register-memory instruction.
    pub(crate) fn encode_rm<T: Reg>(&mut self, opc: u8, op1: T, op2: MemOp)
    where
        Self: EncodeMR<T>,
    {
        // RM operand encoding.
        //   op1 -> modrm.reg
        //   op2 -> modrm.rm
        self.encode_mr(opc, op2, op1);
    }

    /// Encode a jump to label instruction.
    pub(crate) fn encode_jmp_label(&mut self, opc: &[u8], op1: &mut Label) {
        // Emit the opcode.
        self.emit(opc);

        // Record relocation offset starting at the first byte of the disp32.
        op1.record_offset(self.buf.len());

        // Emit a zeroed disp32, which serves as placeholder for the relocation.
        // We currently only support disp32 jump targets.
        self.emit(&[0u8; 4]);

        // Resolve any pending relocations for the label.
        self.resolve(op1);
    }
}

// -- Encoder helper.

/// Encode helper for register-register instructions.
pub(crate) trait EncodeRR<T: Reg> {
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

/// Encode helper for register instructions.
pub(crate) trait EncodeR<T: Reg> {
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

impl EncodeR<Reg8> for Asm {}
impl EncodeR<Reg32> for Asm {}
impl EncodeR<Reg16> for Asm {
    fn legacy_prefix() -> Option<u8> {
        Some(0x66)
    }
}
impl EncodeR<Reg64> for Asm {}

/// Encode helper for memory-register instructions.
pub(crate) trait EncodeMR<T: Reg> {
    fn legacy_prefix() -> Option<u8> {
        None
    }

    fn rex(op1: &MemOp, op2: T) -> Option<u8> {
        if op2.need_rex() || (op1.base().is_ext()) {
            Some(rex(
                op2.rexw(),
                op2.idx(),
                op1.index().idx(),
                op1.base().idx(),
            ))
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

/// Encode helper for memory-immediate instructions.
pub(crate) trait EncodeMI<T: Imm> {
    fn legacy_prefix() -> Option<u8> {
        None
    }

    fn rex(op1: &MemOp) -> Option<u8> {
        if op1.base().is_ext() {
            Some(rex(false, 0, op1.index().idx(), op1.base().idx()))
        } else {
            None
        }
    }
}

impl EncodeMI<Imm8> for Asm {}
impl EncodeMI<Imm16> for Asm {
    fn legacy_prefix() -> Option<u8> {
        Some(0x66)
    }
}
impl EncodeMI<Imm32> for Asm {}
