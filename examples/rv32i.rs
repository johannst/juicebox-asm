// SPDX-License-Identifier: MIT
//
// Copyright (c) 2026, Johannes Stoelp <dev@memzero.de>

//! RISC-V 32bit.
//!
//! This example implements a minimal rv32i userspace emulator with an
//! interpreter and jit compiler to demonstrate the juicebox crate.
//!
//! The emulator only implements a very limited syscall surface, sufficient to
//! run the example software in examples/rv32i-guest/.
//!
//! It aims at simplicity rather than being highly optimized and uses unwraps
//! throughout the implementation to crash the emulator on unexpected behavior.
//!
//! Self-modifying code is not supported and writes to executable memory crash
//! the simulator. In theory the interpreter actually has no problem with
//! self-modifying code as there is no decode cache or anything alike. For the
//! jit one could achieve support by invalidating the translation cache on
//! writes to executable memory and thinking about whether to handle those
//! invalidations as precise or imprecise exceptions in a translation block.

use std::collections::HashMap;
use std::convert::TryFrom;

use juicebox_asm::insn::*;
use juicebox_asm::Runtime;
use juicebox_asm::{Asm, Imm16, Imm32, Imm64, Imm8, Label, Mem16, Mem32, Mem8, Reg32, Reg64, Reg8};

// Enable tracing of different parts of the emulator (mainly for debugging).
const ENABLE_TRACE: bool = false;

macro_rules! trace {
    ($tag:expr, $($arg:tt)*) => ({
        if ENABLE_TRACE {
            print!("{:8}: ", $tag);
            println!($($arg)*);
        }
    });
}

// -- DECODER ------------------------------------------------------------------

type RegIdx = u32;

#[derive(Debug)]
pub struct Rtype {
    rd: RegIdx,
    rs1: RegIdx,
    rs2: RegIdx,
    func3: u32,
    func7: u32,
}

impl From<u32> for Rtype {
    #[rustfmt::skip]
    fn from(insn: u32) -> Self {
        let rd     = (insn >> 7) & 0x1f;
        let func3  = (insn >> 12) & 0x7;
        let rs1    = (insn >> 15) & 0x1f;
        let rs2    = (insn >> 20) & 0x1f;
        let func7  = (insn >> 25) & 0x7f;

        Rtype { rd, rs1, rs2, func3, func7 }
    }
}

#[derive(Debug)]
pub struct Itype {
    rd: RegIdx,
    rs1: RegIdx,
    func3: u32,
    imm: i32,
}

impl From<u32> for Itype {
    #[rustfmt::skip]
    fn from(insn: u32) -> Self {
        let rd     = (insn >> 7) & 0x1f;
        let func3  = (insn >> 12) & 0x7;
        let rs1    = (insn >> 15) & 0x1f;
        let imm    = (insn >> 20) & 0xfff;

        // Sign extend immediate.
        let imm = (imm as i32) << 20 >> 20;

        Itype { rd, rs1, func3, imm }
    }
}

#[derive(Debug)]
pub struct Stype {
    rs1: RegIdx,
    rs2: RegIdx,
    func3: u32,
    imm: i32,
}

impl From<u32> for Stype {
    #[rustfmt::skip]
    fn from(insn: u32) -> Self {
        let imm40  = (insn >> 7) & 0x1f;
        let func3  = (insn >> 12) & 0x7;
        let rs1    = (insn >> 15) & 0x1f;
        let rs2    = (insn >> 20) & 0x1f;
        let imm115 = (insn >> 25) & 0x7f;

        // Construct and sign extend immediate.
        let imm = (imm115 << 5) | imm40;
        let imm = (imm as i32) << 20 >> 20;

        Stype { rs1, rs2, func3, imm }
    }
}

#[derive(Debug)]
pub struct Btype {
    rs1: RegIdx,
    rs2: RegIdx,
    func3: u32,
    imm: i32,
}

impl From<u32> for Btype {
    #[rustfmt::skip]
    fn from(insn: u32) -> Self {
        let imm11  = (insn >> 7) & 0x1;
        let imm41  = (insn >> 8) & 0xf;
        let func3  = (insn >> 12) & 0x7;
        let rs1    = (insn >> 15) & 0x1f;
        let rs2    = (insn >> 20) & 0x1f;
        let imm105 = (insn >> 25) & 0x3f;
        let imm12  = (insn >> 31) & 0x1;

        // Construct and sign extend immediate.
        let imm = (imm12 << 12) | (imm11 << 11) | (imm105 << 5) | (imm41 << 1);
        let imm = (imm as i32) << 19 >> 19;

        Btype { rs1, rs2, func3, imm }
    }
}

#[derive(Debug)]
pub struct Utype {
    rd: RegIdx,
    imm: i32,
}

impl From<u32> for Utype {
    #[rustfmt::skip]
    fn from(insn: u32) -> Self {
        let rd      = (insn >> 7) & 0x1f;
        let imm3112 = (insn >> 12) & 0xfffff;

        // Construct immediate.
        let imm = (imm3112 << 12) as i32;

        Utype { rd, imm }
    }
}

#[derive(Debug)]
pub struct Jtype {
    rd: RegIdx,
    imm: i32,
}

impl From<u32> for Jtype {
    #[rustfmt::skip]
    fn from(insn: u32) -> Self {
        let rd      = (insn >> 7) & 0x1f;
        let imm1912 = (insn >> 12) & 0xff;
        let imm11   = (insn >> 20) & 0x1;
        let imm101  = (insn >> 21) & 0x3ff;
        let imm20   = (insn >> 31) & 0x1;

        // Construct and sign extend immediate.
        let imm = (imm20 << 20) | (imm1912 << 12) | (imm11 << 11) | (imm101 << 1);
        let imm = (imm as i32) << 12 >> 12;

        Jtype { rd, imm }
    }
}

#[rustfmt::skip]
#[derive(Debug)]
pub enum Insn {
    // rv32i
    Lui   { rd: RegIdx, imm: i32 },
    Auipc { rd: RegIdx, imm: i32 },
    Jal   { rd: RegIdx, imm: i32 },
    Jalr  { rd: RegIdx, rs1: RegIdx, imm: i32 },
    Beq   { rs1: RegIdx, rs2: RegIdx, imm: i32 },
    Bne   { rs1: RegIdx, rs2: RegIdx, imm: i32 },
    Blt   { rs1: RegIdx, rs2: RegIdx, imm: i32 },
    Bge   { rs1: RegIdx, rs2: RegIdx, imm: i32 },
    Bltu  { rs1: RegIdx, rs2: RegIdx, imm: i32 },
    Bgeu  { rs1: RegIdx, rs2: RegIdx, imm: i32 },
    Lb    { rd: RegIdx, rs1: RegIdx, imm: i32 },
    Lh    { rd: RegIdx, rs1: RegIdx, imm: i32 },
    Lw    { rd: RegIdx, rs1: RegIdx, imm: i32 },
    Lbu   { rd: RegIdx, rs1: RegIdx, imm: i32 },
    Lhu   { rd: RegIdx, rs1: RegIdx, imm: i32 },
    Sb    { rs1: RegIdx, rs2: RegIdx, imm: i32 },
    Sh    { rs1: RegIdx, rs2: RegIdx, imm: i32 },
    Sw    { rs1: RegIdx, rs2: RegIdx, imm: i32 },
    Addi  { rd: RegIdx, rs1: RegIdx, imm: i32 },
    Slti  { rd: RegIdx, rs1: RegIdx, imm: i32 },
    Sltiu { rd: RegIdx, rs1: RegIdx, imm: i32 },
    Xori  { rd: RegIdx, rs1: RegIdx, imm: i32 },
    Ori   { rd: RegIdx, rs1: RegIdx, imm: i32 },
    Andi  { rd: RegIdx, rs1: RegIdx, imm: i32 },
    Slli  { rd: RegIdx, rs1: RegIdx, shamt: i32 },
    Srli  { rd: RegIdx, rs1: RegIdx, shamt: i32 },
    Srai  { rd: RegIdx, rs1: RegIdx, shamt: i32 },
    Add   { rd: RegIdx, rs1: RegIdx, rs2: RegIdx },
    Sub   { rd: RegIdx, rs1: RegIdx, rs2: RegIdx },
    Sll   { rd: RegIdx, rs1: RegIdx, rs2: RegIdx },
    Slt   { rd: RegIdx, rs1: RegIdx, rs2: RegIdx },
    Sltu  { rd: RegIdx, rs1: RegIdx, rs2: RegIdx },
    Xor   { rd: RegIdx, rs1: RegIdx, rs2: RegIdx },
    Srl   { rd: RegIdx, rs1: RegIdx, rs2: RegIdx },
    Sra   { rd: RegIdx, rs1: RegIdx, rs2: RegIdx },
    Or    { rd: RegIdx, rs1: RegIdx, rs2: RegIdx },
    And   { rd: RegIdx, rs1: RegIdx, rs2: RegIdx },
    Fence { rd: RegIdx, rs1: RegIdx, succ: i32, pred: i32, fm: i32 },
    Ecall,
    Ebreak,

    // rv32a - Zalrsc
    Lr { rd: RegIdx, rs1: RegIdx, aq: bool, rl: bool },
    Sc { rd: RegIdx, rs1: RegIdx, rs2: RegIdx, aq: bool, rl: bool },
}

/// Decode the riscv instruction bytes `insn` into an [`Insn`].
#[rustfmt::skip]
pub fn decode(insn: u32) -> Insn {
    let opcode = insn & 0x7f;
    match opcode {
        0b0110111 => {
            let Utype { rd, imm } = Utype::from(insn);
            Insn::Lui { rd, imm }
        }
        0b0010111 => {
            let Utype { rd, imm } = Utype::from(insn);
            Insn::Auipc { rd, imm }
        }
        0b1101111 => {
            let Jtype { rd, imm } = Jtype::from(insn);
            Insn::Jal { rd, imm }
        }
        0b1100111 => {
            let Itype { rd, rs1, imm, .. } = Itype::from(insn);
            Insn::Jalr { rd, rs1, imm }
        }
        0b1100011 => {
            let Btype { rs1, rs2, func3, imm } = Btype::from(insn);
            match func3 {
                0b000 => Insn::Beq  { rs1, rs2, imm },
                0b001 => Insn::Bne  { rs1, rs2, imm },
                0b100 => Insn::Blt  { rs1, rs2, imm },
                0b101 => Insn::Bge  { rs1, rs2, imm },
                0b110 => Insn::Bltu { rs1, rs2, imm },
                0b111 => Insn::Bgeu { rs1, rs2, imm },
                _ => todo!("branch func3={:b}", func3),
            }
        }
        0b0000011 => {
            let Itype { rd, rs1, func3, imm } = Itype::from(insn);
            match func3 {
                0b000 => Insn::Lb  { rd, rs1, imm },
                0b001 => Insn::Lh  { rd, rs1, imm },
                0b010 => Insn::Lw  { rd, rs1, imm },
                0b100 => Insn::Lbu { rd, rs1, imm },
                0b101 => Insn::Lhu { rd, rs1, imm },
                _ => todo!("load func3={:b}", func3),
            }
        }
        0b0100011 => {
            let Stype { rs1, rs2, func3, imm } = Stype::from(insn);
            match func3 {
                0b000 => Insn::Sb { rs1, rs2, imm },
                0b001 => Insn::Sh { rs1, rs2, imm },
                0b010 => Insn::Sw { rs1, rs2, imm },
                _ => todo!("store func3={:b}", func3),
            }
        }
        0b0010011 => {
            let Itype { rd, rs1, func3, imm } = Itype::from(insn);
            match func3 {
                0b000 => Insn::Addi  { rd, rs1, imm },
                0b010 => Insn::Slti  { rd, rs1, imm },
                0b011 => Insn::Sltiu { rd, rs1, imm },
                0b100 => Insn::Xori  { rd, rs1, imm },
                0b110 => Insn::Ori   { rd, rs1, imm },
                0b111 => Insn::Andi  { rd, rs1, imm },
                0b001 | 0b101 => {
                    let shamt = (imm >> 0) & 0x1f;
                    let sfunc = (imm >> 5) & 0x7f;
                    match func3 {
                        0b001 if sfunc == 0b0000000 => Insn::Slli { rd, rs1, shamt },
                        0b101 if sfunc == 0b0000000 => Insn::Srli { rd, rs1, shamt },
                        0b101 if sfunc == 0b0100000 => Insn::Srai { rd, rs1, shamt },
                        _ => todo!("integer immediate shift func3={:b} sfunc={:b}", func3, sfunc),
                    }
                }
                _ => todo!("integer immediate func3={:b}", func3),
            }
        }
        0b0110011 => {
            let Rtype { rd, rs1, rs2, func3, func7 } = Rtype::from(insn);
            match (func7, func3) {
                (0b0000000, 0b000) => Insn::Add  { rd, rs1, rs2 },
                (0b0100000, 0b000) => Insn::Sub  { rd, rs1, rs2 },
                (0b0000000, 0b001) => Insn::Sll  { rd, rs1, rs2 },
                (0b0000000, 0b010) => Insn::Slt  { rd, rs1, rs2 },
                (0b0000000, 0b011) => Insn::Sltu { rd, rs1, rs2 },
                (0b0000000, 0b100) => Insn::Xor  { rd, rs1, rs2 },
                (0b0000000, 0b101) => Insn::Srl  { rd, rs1, rs2 },
                (0b0100000, 0b101) => Insn::Sra  { rd, rs1, rs2 },
                (0b0000000, 0b110) => Insn::Or   { rd, rs1, rs2 },
                (0b0000000, 0b111) => Insn::And  { rd, rs1, rs2 },
                _ => todo!("integer register func3={:b} func7={:b}", func3, func7),
            }
        }
        0b0001111 => {
            let Itype { rd, rs1, imm, .. } = Itype::from(insn);
            let succ = (imm >> 0) & 0xf;
            let pred = (imm >> 4) & 0xf;
            let fm = (imm >> 8) & 0xf;
            Insn::Fence { rd, rs1, succ, pred, fm }
        }
        0b1110011 => {
            let Itype { rd, rs1, func3, imm } = Itype::from(insn);

            match (func3, imm) {
                (0b000, 0) => {
                    assert_eq!(rs1, 0);
                    assert_eq!(rd,  0);
                    Insn::Ecall
                }
                (0b000, 1) => {
                    assert_eq!(rs1, 0);
                    assert_eq!(rd,  0);
                    Insn::Ebreak
                }
                _ => todo!("system instruction func3={:b}", func3),
            }
        },
        0b0101111 => {
            let Rtype { rd, rs1, rs2, func3, func7 } = Rtype::from(insn);
            let func5 = func7 >> 2;
            let rl = func7 & (1 << 0) != 0;
            let aq = func7 & (1 << 1) != 0;

            match (func5, func3) {
                (0b00010, 0b010) => Insn::Lr { rd, rs1, aq, rl },
                (0b00011, 0b010) => Insn::Sc { rd, rs1, rs2, aq, rl },
                _ => todo!("amo insutrction func5={:b} func3={:b}", func5, func3),
            }
        }
        _ => todo!("instruction=0x{:08x} op=0b{:07b}", insn, opcode),
    }
}

// -- GUEST STATE --------------------------------------------------------------

/// Register ABI name - Stack Pointer.
pub const SP: RegIdx = 2;
/// Register ABI name - Argument 0.
pub const A0: RegIdx = 10;
/// Register ABI name - Argument 1.
pub const A1: RegIdx = 11;
/// Register ABI name - Argument 2.
pub const A2: RegIdx = 12;
/// Register ABI name - Argument 7.
pub const A7: RegIdx = 17;

/// Protection flag - read access.
pub const PROT_R: u8 = 1 << 0;
/// Protection flag - write access.
pub const PROT_W: u8 = 1 << 1;
/// Protection flag - execute access.
pub const PROT_X: u8 = 1 << 2;

#[derive(Debug)]
pub enum ExitReason {
    /// Exit when the guest is about to execute an `ecall` instruction.
    /// The pc of the guest on exit points to the `ecall` instruction.
    Ecall,

    /// Exit when the guest is about to execute an `ebreak` instruction.
    /// The pc of the guest on exit points to the `ebreak` instruction.
    Ebreak,
}

/// The rv32i guest state.
pub struct GuestState {
    /// General purpose register of the riscv hart.
    regs: [u32; 32],

    /// Program counter.
    pc: u32,

    /// Program counter to be used when restarting the guest after an exit.
    reenter_pc: Option<u32>,

    /// Virtual memory for the guest program.
    vmem: Vec<u8>,

    /// Virtual memory protection (byte-level precision).
    prot: Vec<u8>,

    // Jit state.
    /// Translation block cache mapping from guest PCs to translated code
    /// blocks. For simplicity just a hasmap.
    tb_cache: HashMap<u32, JitFn>,

    /// The jit runtime, holding the generated host code.
    rt: Runtime,
}

/// Function signature for a jit compiled translation block.
pub type JitFn = unsafe extern "C" fn(regs: *mut u32, mem: *mut u8, prot: *const u8) -> JitRet;

/// Jit function return value.
#[repr(C)]
pub struct JitRet {
    /// Exit reason when returning from the jit compiled translation block.
    exit_reason: u64,

    /// Program counter to be used when restarting the guest after an exit.
    reenter_pc: u64,
}

/// Jit exit when the end of a translation block is reached.
pub const JIT_TB_END: u32 = 0;
/// Jit exit when the guest is about to execute an `ecall` instruction.
pub const JIT_ECALL: u32 = 1;
/// Jit exit when the guest is about to execute an `ebreak` instruction.
pub const JIT_EBREAK: u32 = 2;
/// Jit exit when a load instruction faults due to an out of bounds access or
/// read protection violation.
pub const JIT_LD_FAULT: u32 = 3;
/// Jit exit when a store instruction faults due to an out of bounds access or
/// write protection violation.
pub const JIT_ST_FAULT: u32 = 4;

macro_rules! mem_write {
    ($self: expr, $ty: ty, $ea: expr, $data: expr) => {{
        $self.write_mem($ea, &($data as $ty).to_le_bytes());
    }};
}

macro_rules! mem_read {
    ($self: expr, $ty: ty, $ea: expr) => {{
        let mut bytes = [0u8; core::mem::size_of::<$ty>()];
        $self.read_mem($ea, &mut bytes);
        <$ty>::from_le_bytes(bytes)
    }};
}

impl GuestState {
    /// Create a new guest state with the virtual address space `[0..mem_size)`.
    /// The virtual address space is initially unmapped, and accessing it from
    /// the guest will raise a fault.
    pub fn new(mem_size: usize) -> Self {
        assert!(mem_size <= 4 * 1024 * 1024 * 1024);
        let mut vmem = Vec::with_capacity(mem_size);
        vmem.resize(vmem.capacity(), 0);

        let mut prot = Vec::with_capacity(mem_size);
        prot.resize(prot.capacity(), 0);

        GuestState {
            regs: [0; 32],
            pc: 0,
            reenter_pc: None,
            vmem,
            prot,
            tb_cache: HashMap::new(),
            rt: Runtime::with_capacity(32),
        }
    }

    /// Read the guest register `r`.
    pub fn read_reg(&self, r: RegIdx) -> u32 {
        if r == 0 {
            return 0;
        }

        let idx = r as usize;
        debug_assert!(idx < self.regs.len());
        self.regs[idx]
    }

    /// Write `val` to the guest register `r`.
    pub fn write_reg(&mut self, r: RegIdx, val: u32) {
        if r == 0 {
            return;
        }

        let idx = r as usize;
        debug_assert!(idx < self.regs.len());
        self.regs[idx] = val;
    }

    /// Check if the address range `[addr..addr+len)` has at least the `prot`
    /// protection flag set.
    pub fn check_prot(&self, addr: u32, len: usize, prot: u8) -> bool {
        let start = addr as usize;
        let end = start.checked_add(len).unwrap();
        self.prot
            .get(start..end)
            .unwrap()
            .iter()
            .any(|p| (p & prot) != prot)
    }

    /// Set the `prot` protection flag for the address range `[addr..addr+len)`.
    /// This will just overwrite the current protection flags, and does not
    /// check if there were other protection flags set.
    pub fn set_prot(&mut self, addr: u32, len: usize, prot: u8) {
        let start = addr as usize;
        let end = start.checked_add(len).unwrap();
        self.prot
            .get_mut(start..end)
            .unwrap()
            .iter_mut()
            .for_each(|p| *p = prot);
    }

    /// Map the virtual address range `[addr..addr+data.len)` with the
    /// protection provided in `prot` and initialize the memory with `data`.
    pub fn map_mem(&mut self, addr: u32, data: &[u8], prot: u8) {
        let start = addr as usize;
        let end = start.checked_add(data.len()).unwrap();
        self.set_prot(addr, data.len(), prot);
        self.vmem.get_mut(start..end).unwrap().copy_from_slice(data);

        println!(
            "MAP DATA: vaddr: 0x{:08x} len: {:5} {}{}{}",
            addr,
            data.len(),
            if prot & PROT_X != 0 { 'X' } else { '-' },
            if prot & PROT_W != 0 { 'W' } else { '-' },
            if prot & PROT_R != 0 { 'R' } else { '-' },
        );
    }

    /// Map the virtual address range `[addr..addr+len)` with the protection
    /// provided in `prot` and initialize the memory with `0`.
    pub fn map_mem_zero(&mut self, addr: u32, len: usize, prot: u8) {
        let start = addr as usize;
        let end = start.checked_add(len).unwrap();
        self.set_prot(addr, len, prot);
        self.vmem.get_mut(start..end).unwrap().fill(0);

        println!(
            "MAP ZERO: vaddr: 0x{:08x} len: {:5} {}{}{}",
            addr,
            len,
            if prot & PROT_X != 0 { 'X' } else { '-' },
            if prot & PROT_W != 0 { 'W' } else { '-' },
            if prot & PROT_R != 0 { 'R' } else { '-' },
        );
    }

    /// Write `data` to the virtual address range `[addr..addr+data.len)`.
    /// This performs a check if the address range has the write protection set.
    pub fn write_mem(&mut self, addr: u32, data: &[u8]) {
        let start = addr as usize;
        let end = start.checked_add(data.len()).unwrap();

        let wfault = self.check_prot(addr, data.len(), PROT_W);
        assert!(!wfault, "write_fault @0x{:08x} len={}", addr, data.len());

        // Check for self-modifying code.
        let xfault = self.check_prot(addr, data.len(), PROT_X);
        assert!(xfault, "write_fault @0x{:08x} len={} smc", addr, data.len());

        self.vmem.get_mut(start..end).unwrap().copy_from_slice(data);
    }

    /// Read from the virtual address range `[addr..addr+data.len)` into
    /// `data`. This performs a check if the address range has the read
    /// protection set.
    pub fn read_mem(&self, addr: u32, data: &mut [u8]) {
        let start = addr as usize;
        let end = start.checked_add(data.len()).unwrap();

        let rfault = self.check_prot(addr, data.len(), PROT_R);
        assert!(!rfault, "read_fault @0x{:08x} len={}", addr, data.len());

        data.copy_from_slice(&self.vmem.get(start..end).unwrap());
    }

    /// Get a slice for the virtual address range `[addr..addr+len)`. This
    /// performs a check if the address range has the read protection set.
    pub fn slice_mem(&self, addr: u32, len: usize) -> &[u8] {
        let start = addr as usize;
        let end = start.checked_add(len).unwrap();

        let rfault = self.check_prot(addr, len, PROT_R);
        assert!(!rfault, "read_fault @0x{:08x} len={}", addr, len);

        self.vmem.get(start..end).unwrap()
    }

    /// Fetch an instruction from `pc`. This performs a check if the address is
    /// properly aligned and the range has the exec protection set.
    pub fn fetch_insn(&self, pc: u32) -> u32 {
        debug_assert_eq!(pc & 0b11, 0, "PC must be 4byte aligned!");

        let xfault = self.check_prot(pc, 4, PROT_X);
        assert!(!xfault, "exec_fault @0x{:08x} len={}", pc, 4);

        mem_read!(self, u32, pc)
    }

    /// Execute guest on interpreter until an [`ExitReason`] is hit, starting
    /// from `self.pc`.
    pub fn interpret(&mut self) -> ExitReason {
        if let Some(pc) = self.reenter_pc.take() {
            self.pc = pc;
        }

        loop {
            // Decode current instruction.
            let insn = self.fetch_insn(self.pc);
            let insn = decode(insn);
            trace!("interp", "{:08x} {:?}", self.pc, insn);

            // Interpret current instruction.
            let exit = self.interpret_insn(insn);

            match exit {
                // Update pc after branch/jump.
                Ok(Some(pc)) => self.pc = pc,
                // Increment pc after normal instruction.
                Ok(None) => self.pc = self.pc.wrapping_add(4),
                // Exit with observed exit reason.
                Err(r) => {
                    // PC used when restarting after an exit.
                    self.reenter_pc = Some(self.pc.wrapping_add(4));
                    return r;
                }
            }
        }
    }

    /// Interpret a single guest instruction `insn`.
    #[rustfmt::skip]
    pub fn interpret_insn(&mut self, insn: Insn) -> Result<Option<u32>, ExitReason> {
        match insn {
            Insn::Lui { rd, imm } => {
                let res = imm;
                self.write_reg(rd, res as u32);
            }
            Insn::Auipc { rd, imm } => {
                let res = self.pc as i32 + imm;
                self.write_reg(rd, res as u32);
            }
            Insn::Jal { rd, imm } => {
                // Save return address.
                self.write_reg(rd, self.pc + 4);

                // Update PC with jump target.
                let pc = self.pc as i32 + imm;
                debug_assert_eq!(self.pc & 0b11, 0, "Instruction misaligned exception!");

                return Ok(Some(pc as u32));
            }
            Insn::Jalr { rd, rs1, imm } => {
                let rs1 = self.read_reg(rs1);

                // Save return address.
                self.write_reg(rd, self.pc + 4);

                // Update PC with jump target.
                let pc = rs1 as i32 + imm & !1;
                debug_assert_eq!(self.pc & 0b11, 0, "Instruction misaligned exception!");

                return Ok(Some(pc as u32));
            }
            Insn::Beq  { rs1, rs2, imm } |
            Insn::Bne  { rs1, rs2, imm } |
            Insn::Blt  { rs1, rs2, imm } |
            Insn::Bge  { rs1, rs2, imm } |
            Insn::Bltu { rs1, rs2, imm } |
            Insn::Bgeu { rs1, rs2, imm } => {
                let rs1 = self.read_reg(rs1);
                let rs2 = self.read_reg(rs2);

                let take = match insn {
                    Insn::Beq  { .. } => rs1 == rs2,
                    Insn::Bne  { .. } => rs1 != rs2,
                    Insn::Blt  { .. } => (rs1 as i32) < (rs2 as i32),
                    Insn::Bge  { .. } => (rs1 as i32) >= (rs2 as i32),
                    Insn::Bltu { .. } => rs1 < rs2,
                    Insn::Bgeu { .. } => rs1 >= rs2,
                    _ => {
                        unreachable!()
                    }
                };

                if take {
                    let pc = self.pc as i32 + imm;
                    debug_assert_eq!(self.pc & 0b11, 0, "Instruction misaligned exception!");

                    return Ok(Some(pc as u32));
                }
            }
            Insn::Lb  { rd, rs1, imm } |
            Insn::Lh  { rd, rs1, imm } |
            Insn::Lw  { rd, rs1, imm } |
            Insn::Lbu { rd, rs1, imm } |
            Insn::Lhu { rd, rs1, imm } => {
                let rs1 = self.read_reg(rs1) as i32;
                let ea = rs1.wrapping_add(imm) as u32;

                let ret = match insn {
                    Insn::Lb  { .. } => mem_read!(self, u8 , ea) as i8  as i32,
                    Insn::Lh  { .. } => mem_read!(self, u16, ea) as i16 as i32,
                    Insn::Lw  { .. } => mem_read!(self, u32, ea) as i32,
                    Insn::Lbu { .. } => mem_read!(self, u8 , ea) as u32 as i32,
                    Insn::Lhu { .. } => mem_read!(self, u16, ea) as u32 as i32,
                    _ => unreachable!(),
                };

                self.write_reg(rd, ret as u32);
            }
            Insn::Sb { rs1, rs2, imm } |
            Insn::Sh { rs1, rs2, imm } |
            Insn::Sw { rs1, rs2, imm } => {
                let rs1 = self.read_reg(rs1) as i32;
                let rs2 = self.read_reg(rs2);
                let ea = rs1.wrapping_add(imm) as u32;

                match insn {
                    Insn::Sb { .. } => mem_write!(self, u8 , ea, rs2),
                    Insn::Sh { .. } => mem_write!(self, u16, ea, rs2),
                    Insn::Sw { .. } => mem_write!(self, u32, ea, rs2),
                    _ => unreachable!(),
                }
            }
            Insn::Addi  { rd, rs1, imm } |
            Insn::Slti  { rd, rs1, imm } |
            Insn::Sltiu { rd, rs1, imm } |
            Insn::Xori  { rd, rs1, imm } |
            Insn::Ori   { rd, rs1, imm } |
            Insn::Andi  { rd, rs1, imm } |
            Insn::Slli  { rd, rs1, shamt: imm } |
            Insn::Srli  { rd, rs1, shamt: imm } |
            Insn::Srai  { rd, rs1, shamt: imm } => {
                let rs1u = self.read_reg(rs1);
                let rs1s = rs1u as i32;
                let imm = imm;

                let res = match insn {
                    Insn::Addi  { .. } => rs1s.wrapping_add(imm),
                    Insn::Slti  { .. } => if rs1s < imm { 1 } else { 0 },
                    Insn::Sltiu { .. } => if rs1u < imm as u32 { 1 } else { 0 },
                    Insn::Xori  { .. } => rs1s ^ imm,
                    Insn::Ori   { .. } => rs1s | imm,
                    Insn::Andi  { .. } => rs1s & imm,
                    Insn::Slli  { .. } => (rs1u << imm) as i32,
                    Insn::Srli  { .. } => (rs1u >> imm) as i32,
                    Insn::Srai  { .. } => rs1s >> imm,
                    _ => {
                        unreachable!()
                    }
                };

                self.write_reg(rd, res as u32);
            }
            Insn::Add  { rd, rs1, rs2 } |
            Insn::Sub  { rd, rs1, rs2 } |
            Insn::Sll  { rd, rs1, rs2 } |
            Insn::Slt  { rd, rs1, rs2 } |
            Insn::Sltu { rd, rs1, rs2 } |
            Insn::Xor  { rd, rs1, rs2 } |
            Insn::Srl  { rd, rs1, rs2 } |
            Insn::Sra  { rd, rs1, rs2 } |
            Insn::Or   { rd, rs1, rs2 } |
            Insn::And  { rd, rs1, rs2 } => {
                let rs1 = self.read_reg(rs1);
                let rs2 = self.read_reg(rs2);
                let shamt = rs2 & 0x1f;

                let res = match insn {
                    Insn::Add  { .. } => rs1.wrapping_add(rs2),
                    Insn::Sub  { .. } => rs1.wrapping_sub(rs2),
                    Insn::Sll  { .. } => rs1 << shamt,
                    Insn::Slt  { .. } => if (rs1 as i32) < (rs2 as i32) { 1 } else { 0 },
                    Insn::Sltu { .. } => if rs1 < rs2 { 1 } else { 0 },
                    Insn::Xor  { .. } => rs1 ^ rs2,
                    Insn::Srl  { .. } => rs1 >> shamt,
                    Insn::Sra  { .. } => ((rs1 as i32) >> shamt) as u32,
                    Insn::Or   { .. } => rs1 | rs2,
                    Insn::And  { .. } => rs1 & rs2,
                    _ => {
                        unreachable!()
                    }
                };

                self.write_reg(rd, res as u32);
            }
            Insn::Fence { rd, rs1, succ, pred, fm } => todo!("fence rd={rd} rs1={rs1} succ={succ} pred={pred} fm={fm}"),
            Insn::Ecall =>  return Err(ExitReason::Ecall),
            Insn::Ebreak => return Err(ExitReason::Ebreak),
            Insn::Lr { rd, rs1, .. } => {
                // Implement LR as a simple load w/o the reservation set to
                // implement the exclusive access. Also make the blunt
                // assumption that sw "behaves" and does not break the exclusive
                // access on a single thread.

                let rs1 = self.read_reg(rs1);
                assert!(rs1 % 4 == 0, "LR address misaligned exception!");

                let ret = mem_read!(self, u32, rs1);
                self.write_reg(rd, ret);
            }
            Insn::Sc { rd, rs1, rs2, .. } => {
                // Implement SC as a simple store w/o the reservation set to
                // implement the exclusive access. Also make the blunt
                // assumption that sw "behaves" and does not break the exclusive
                // access on a single thread.

                let rs1 = self.read_reg(rs1);
                assert!(rs1 % 4 == 0, "SC address misaligned exception!");

                let rs2 = self.read_reg(rs2);
                mem_write!(self, u32, rs1, rs2);

                // Return that Sc was successful.
                self.write_reg(rd, 0);
            }
        }

        Ok(None)
    }

    /// Execute guest from jit compiled code until an [`ExitReason`] is hit,
    /// starting from `self.pc`.
    pub fn jit(&mut self) -> ExitReason {
        loop {
            if let Some(pc) = self.reenter_pc.take() {
                self.pc = pc;
            }

            // Lookup TB function or compile next translation block.
            let tb_fn = match self.tb_cache.get(&self.pc) {
                Some(bb_fn) => *bb_fn,
                None => {
                    let tb_fn = self.translate_next_block();
                    self.tb_cache.insert(self.pc, tb_fn);
                    trace!(
                        "comp",
                        "translate block pc={:08x} -> tb_fn={:x}",
                        self.pc,
                        tb_fn as usize
                    );
                    tb_fn
                }
            };

            // Call into jit compiled code for TB.
            let ret = unsafe {
                tb_fn(
                    self.regs.as_mut_ptr(),
                    self.vmem.as_mut_ptr(),
                    self.prot.as_ptr(),
                )
            };

            self.reenter_pc = Some(ret.reenter_pc as u32);

            match ret.exit_reason as u32 {
                JIT_TB_END => {}
                JIT_ECALL => return ExitReason::Ecall,
                JIT_EBREAK => return ExitReason::Ebreak,
                JIT_LD_FAULT => todo!("jit load fault"),
                JIT_ST_FAULT => todo!("jit store fault"),
                r @ _ => todo!("jit unhandled exit {r}"),
            }
        }
    }

    /// Translate the next block of guest code at `self.pc` and return a
    /// [`JitFn`] to the compiled TB.
    #[cfg(all(any(target_arch = "x86_64", target_os = "linux")))]
    #[rustfmt::skip]
    pub fn translate_next_block(&mut self) -> JitFn {
        let mut tb = Asm::new();
        let mut pc = self.pc;

        // The jit abi is as follows: JitFn -> JitRet.
        //
        // For passing and returning values to and from the JitFn the SystemV
        // abi is assumed.
        //
        // Throughout the execution of the TB the guest state is accessible via
        // the following registers.
        //   rdi => ptr to regs
        //   r8  => ptr to vmem
        //   r9  => ptr to prot
        //
        // The return value must be passed via the following registers.
        //   eax => exit_code
        //   edx => reenter_pc
        // > This intentionally uses 32 bit register while the JitRet is using
        // > u64 values, as 32 bit registers are zero-extended to 64 bit values.
        //
        // The jit compiler follows the design choice to strongly use
        // caller-saved register when emitting code. This removes the need of
        // saving and restoring registers when entering and exiting a TB.
        //
        // When the jit compiler calls out to a C function however, it needs to
        // save and restore caller-saved register alive at this time.
        //
        // On x86_64 with the SystemV abi the following registers are
        // caller-saved: rcx, rdx, rsi, rdi, rsp, r8 - r11.

        // Go from SystemV abi to the jit abi.
        tb.mov(Reg64::r8, Reg64::rsi); // Ptr to vmem.
        tb.mov(Reg64::r9, Reg64::rdx); // Ptr to prot.

        // Host register available for the register allocator.
        let mut host_regs = vec![Reg32::r10d, Reg32::edx, Reg32::esi];

        'outer: loop {
            // -- TRANSLATION UTILS --------------------------------------------

            // Allocate a host register.
            let alloc_hostreg = |alloc: &mut Vec<Reg32>| -> Reg32 {
                alloc.pop().expect("out of host register to allocate")
            };
            // Free a host register.
            let free_hostreg = |alloc: &mut Vec<Reg32>, hreg: Reg32| {
                alloc.push(hreg);
            };

            // Generate a memory operand for the guest register `r`.
            let reg_op = |r: RegIdx| Mem32::indirect_disp(Reg64::rdi, (r * 4).try_into().unwrap());

            // Emit load guest register `rs` into host register. This allocates
            // a host register and returns the allocated host register.
            let emit_load_reg = |tb: &mut Asm, alloc: &mut Vec<Reg32>, rs: RegIdx| -> Reg32 {
                let rs_hreg = alloc_hostreg(alloc);
                if rs != 0 {
                    tb.mov(rs_hreg, reg_op(rs));
                } else {
                    // Zero host register when reading from guest zero register,
                    // saving the load from the guest reigster file.
                    tb.xor(rs_hreg, rs_hreg);
                }
                rs_hreg
            };

            // Emit store host register `rs_hreg` into guest register `rd`. This
            // frees the host register `rs_hreg` and hence consumes it.
            let emit_store_reg = |tb: &mut Asm, alloc: &mut Vec<Reg32>, rd: RegIdx, rs_hreg: Reg32| {
                if rd != 0 {
                    // Ignore stores into the zero register.
                    tb.mov(reg_op(rd), rs_hreg);
                }
                free_hostreg(alloc, rs_hreg);
            };

            // Emit store immediate `imm` into guest register `rd`.
            let emit_store_reg_imm = |tb: &mut Asm, rd: RegIdx, imm: u32| {
                if rd != 0 {
                    // Ignore stores into the zero register.
                    tb.mov(reg_op(rd), Imm32::from(imm));
                }
            };

            // Emit a jit return with the `reason` and the reenter pc `next_pc`.
            let emit_ret_imm = |tb: &mut Asm, reason: u32, next_pc: u32| {
                tb.mov(Reg32::eax, Imm32::from(reason));
                tb.mov(Reg32::edx, Imm32::from(next_pc));
                tb.ret();
            };

            // Emit a jit return with the `reason` and the reenter pc `next_pc`.
            let emit_ret_reg = |tb: &mut Asm, reason: u32, next_pc: Reg32| {
                tb.mov(Reg32::eax, Imm32::from(reason));
                if !matches!(next_pc, Reg32::edx) {
                    tb.mov(Reg32::edx, next_pc);
                }
                tb.ret();
            };

            // -- TRANSLATION BEGIN --------------------------------------------

            debug_assert_eq!(pc & 0b11, 0, "PC must be 4byte aligned!");

            // Decode current instruction.
            let insn = self.fetch_insn(pc);
            let insn = decode(insn);

            // If enabled, emit instruction trace in the jit compiled code. The
            // trace is emitted before each instruction is executed.
            if ENABLE_TRACE {
                extern "C" fn jit_insn_trace(ctx: *const GuestState, pc: u32) {
                    let guest: &GuestState = unsafe { &*ctx };
                    let insn = guest.fetch_insn(pc);
                    let insn = decode(insn);
                    trace!("jit", "{:08x} {:?}", guest.pc, insn);
                }

                // At this point only the global guest state is alive since the
                // compiler is between two guest instructions.
                //
                // Save the global guest state held in caller-saved registers as
                // for the instruction trace a function call to a C function is
                // emitted.
                tb.push(Reg64::rdi);
                tb.push(Reg64::r8);
                tb.push(Reg64::r9);
                // The RSP is now 16 byte aligned as required by the CALL
                // instruction on x86.
                //
                // Since the generated TB does currently not make use of the
                // stack across guest instructions, its stack pointer on
                // entrance has a trailing 0x...8. This is guaranteed because
                // when calling into the TB the CALL instruction pushed the RET
                // value on the stack.

                // Prepare the function arguments according to the SystemV abi.
                tb.mov(Reg64::rdi, Imm64::from(self as *const GuestState as usize));
                tb.mov(Reg32::esi, Imm32::from(pc));
                // Emit call to the instruction trace function.
                tb.mov(Reg64::rax, Imm64::from(jit_insn_trace as *const () as usize));
                tb.call(Reg64::rax);

                // Restore registers with the global guest state.
                tb.pop(Reg64::r9);
                tb.pop(Reg64::r8);
                tb.pop(Reg64::rdi);
            }

            match insn {
                Insn::Lui { rd, imm } => {
                    emit_store_reg_imm(&mut tb, rd, imm as u32);
                }
                Insn::Auipc { rd, imm } => {
                    let res = (pc as i32).wrapping_add(imm) as u32;
                    emit_store_reg_imm(&mut tb, rd, res);
                }
                Insn::Jal { rd, imm } => {
                    // Save return address.
                    let ret_pc = pc.wrapping_add(4);
                    emit_store_reg_imm(&mut tb, rd, ret_pc);

                    // Update PC with jump target.
                    let next_pc = (pc as i32).wrapping_add(imm) as u32;
                    debug_assert_eq!(self.pc & 0b11, 0, "Instruction misaligned exception!");

                    emit_ret_imm(&mut tb, JIT_TB_END, next_pc);
                    break 'outer;
                }
                Insn::Jalr { rd, rs1, imm } => {
                    // First load registers as rd could be equal to rs1 and this
                    // should read the register value before rd is updated with
                    // the return value.
                    let reg = emit_load_reg(&mut tb, &mut host_regs, rs1);

                    emit_store_reg_imm(&mut tb, rd, pc.wrapping_add(4));

                    // Compute next pc and emit a jit exit.
                    tb.add(reg, Imm32::from(imm & !1));
                    emit_ret_reg(&mut tb, JIT_TB_END, reg);

                    free_hostreg(&mut host_regs, reg);
                    break 'outer;
                }
                Insn::Beq  { rs1, rs2, imm } |
                Insn::Bne  { rs1, rs2, imm } |
                Insn::Blt  { rs1, rs2, imm } |
                Insn::Bge  { rs1, rs2, imm } |
                Insn::Bltu { rs1, rs2, imm } |
                Insn::Bgeu { rs1, rs2, imm } => {
                    let reg1 = emit_load_reg(&mut tb, &mut host_regs, rs1);
                    let reg2 = emit_load_reg(&mut tb, &mut host_regs, rs2);

                    let mut not_taken = Label::new();
                    tb.cmp(reg1, reg2);
                    match insn {
                        Insn::Beq  { .. } => tb.jnz(&mut not_taken),
                        Insn::Bne  { .. } => tb.jz(&mut not_taken),
                        Insn::Blt  { .. } => tb.jge(&mut not_taken),
                        Insn::Bge  { .. } => tb.jl(&mut not_taken),
                        Insn::Bltu { .. } => tb.jae(&mut not_taken),
                        Insn::Bgeu { .. } => tb.jb(&mut not_taken),
                        i @ _ => unreachable!("{i:?}"),
                    };

                    // True target return.
                    let taken_pc = (pc as i32).wrapping_add(imm) as u32;
                    emit_ret_imm(&mut tb, JIT_TB_END, taken_pc);

                    // False target return.
                    tb.bind(&mut not_taken);
                    let notaken_pc = (pc as i32).wrapping_add(4) as u32;
                    emit_ret_imm(&mut tb, JIT_TB_END, notaken_pc);

                    free_hostreg(&mut host_regs, reg1);
                    free_hostreg(&mut host_regs, reg2);

                    break 'outer;
                }
                Insn::Lb  { rd, rs1, imm } |
                Insn::Lh  { rd, rs1, imm } |
                Insn::Lw  { rd, rs1, imm } |
                Insn::Lbu { rd, rs1, imm } |
                Insn::Lhu { rd, rs1, imm } => {
                    debug_assert!(rd != 0, "Load into zero register unsupported!");

                    // Compute effective address -> rs1 + imm.
                    let reg1 = emit_load_reg(&mut tb, &mut host_regs, rs1);
                    if imm != 0 {
                        tb.add(reg1, Imm32::from(imm));
                    }

                    let mut check = Label::new();
                    let mut fault = Label::new();

                    // Check if effective address is out of bounds of the guest vmem.
                    tb.cmp(reg1, Imm32::from(self.vmem.len() as u32));
                    tb.jb(&mut check);

                    // Emit exit block for load faults.
                    tb.bind(&mut fault);

                    emit_ret_imm(&mut tb, JIT_LD_FAULT, pc + 4);
                    tb.bind(&mut check);

                    match insn {
                        Insn::Lb  { .. } => {
                            // Check memory permission bits for byte (u8) and emit fault if permissions don't match.
                            let reg2 = alloc_hostreg(&mut host_regs);
                            let reg2b = reg2.narrow().narrow();
                            tb.mov(reg2b, Mem8::indirect_base_index(Reg64::r9, reg1.wider()));
                            tb.and(reg2b, Imm8::from(PROT_R));
                            tb.cmp(reg2b, Imm8::from(PROT_R));
                            tb.jnz(&mut fault);
                            free_hostreg(&mut host_regs, reg2);

                            // Load byte from guest vmem and sign-extend.
                            tb.movsx(reg1, Mem8::indirect_base_index(Reg64::r8, reg1.wider()));
                        },
                        Insn::Lh  { .. } => {
                            // Check memory permission bits for half-word (u16) and emit fault if permissions don't match.
                            let reg2 = alloc_hostreg(&mut host_regs);
                            let reg2h = reg2.narrow();
                            tb.mov(reg2h, Mem16::indirect_base_index(Reg64::r9, reg1.wider()));
                            tb.and(reg2h, Imm16::splat_u8(PROT_R));
                            tb.cmp(reg2h, Imm16::splat_u8(PROT_R));
                            tb.jnz(&mut fault);
                            free_hostreg(&mut host_regs, reg2);

                            // Load half-word from guest vmem and sign-extend.
                            tb.movsx(reg1, Mem16::indirect_base_index(Reg64::r8, reg1.wider()));
                        },
                        Insn::Lw  { .. } => {
                            // Check memory permission bits for word (u32) and emit fault if permissions don't match.
                            let reg2 = alloc_hostreg(&mut host_regs);
                            tb.mov(reg2, Mem32::indirect_base_index(Reg64::r9, reg1.wider()));
                            tb.and(reg2, Imm32::splat_u8(PROT_R));
                            tb.cmp(reg2, Imm32::splat_u8(PROT_R));
                            tb.jnz(&mut fault);
                            free_hostreg(&mut host_regs, reg2);

                            // Load word from guest vmem.
                            tb.mov(reg1, Mem32::indirect_base_index(Reg64::r8, reg1.wider()));
                        },
                        Insn::Lbu { .. } => {
                            // Check memory permission bits for byte (u8) and emit fault if permissions don't match.
                            let reg2 = alloc_hostreg(&mut host_regs);
                            let reg2b = reg2.narrow().narrow();
                            tb.mov(reg2b, Mem8::indirect_base_index(Reg64::r9, reg1.wider()));
                            tb.and(reg2b, Imm8::from(PROT_R));
                            tb.cmp(reg2b, Imm8::from(PROT_R));
                            tb.jnz(&mut fault);
                            free_hostreg(&mut host_regs, reg2);

                            // Load byte from guest vmem and zero-extend.
                            tb.movzx(reg1, Mem8::indirect_base_index(Reg64::r8, reg1.wider()));
                        },
                        Insn::Lhu { .. } => {
                            // Check memory permission bits for half-word (u16) and emit fault if permissions don't match.
                            let reg2 = alloc_hostreg(&mut host_regs);
                            let reg2h = reg2.narrow();
                            tb.mov(reg2h, Mem16::indirect_base_index(Reg64::r9, reg1.wider()));
                            tb.and(reg2h, Imm16::splat_u8(PROT_R));
                            tb.cmp(reg2h, Imm16::splat_u8(PROT_R));
                            tb.jnz(&mut fault);
                            free_hostreg(&mut host_regs, reg2);

                            // Load half-word from guest vmem and zero-extend.
                            tb.movzx(reg1, Mem16::indirect_base_index(Reg64::r8, reg1.wider()));
                        },
                        i @ _ => unreachable!("{i:?}"),
                    };

                    // Save word in register.
                    emit_store_reg(&mut tb, &mut host_regs, rd, reg1);
                }
                Insn::Sb { rs1, rs2, imm } |
                Insn::Sh { rs1, rs2, imm } |
                Insn::Sw { rs1, rs2, imm } => {
                    // Compute effective address -> rs1 + imm.
                    let reg1 = emit_load_reg(&mut tb, &mut host_regs, rs1);
                    if imm != 0 {
                        tb.add(reg1, Imm32::from(imm));
                    }

                    let mut check = Label::new();
                    let mut fault = Label::new();

                    // Check if effective address is out of bounds of the guest vmem.
                    tb.cmp(reg1, Imm32::from(self.vmem.len() as u32));
                    tb.jb(&mut check);

                    // Emit exit block for load faults.
                    tb.bind(&mut fault);
                    emit_ret_imm(&mut tb, JIT_ST_FAULT, pc + 4);
                    tb.bind(&mut check);

                    match insn {
                        Insn::Sb { .. } => {
                            // Check memory permission bits for byte (u8) and emit fault if permissions don't match.
                            let reg2 = alloc_hostreg(&mut host_regs);
                            let reg2b = reg2.narrow().narrow();
                            tb.mov(reg2b, Mem8::indirect_base_index(Reg64::r9, reg1.wider()));
                            tb.and(reg2b, Imm8::from(PROT_W));
                            tb.cmp(reg2b, Imm8::from(PROT_W));
                            tb.jnz(&mut fault);

                            // Check if writing to executable memory, if so then fault. No support for potentially self-modifying code.
                            tb.mov(reg2b, Mem8::indirect_base_index(Reg64::r9, reg1.wider()));
                            tb.and(reg2b, Imm8::from(PROT_X));
                            tb.test(reg2b, reg2b);
                            tb.jnz(&mut fault);
                            free_hostreg(&mut host_regs, reg2);

                            // Load byte to store from guest reg.
                            let reg2 = emit_load_reg(&mut tb, &mut host_regs, rs2);
                            let reg2b = reg2.narrow().narrow();

                            // Store word in guest vmem.
                            tb.mov(Mem8::indirect_base_index(Reg64::r8, reg1.wider()), reg2b);

                            free_hostreg(&mut host_regs, reg2);
                        },
                        Insn::Sh { .. } => {
                            // Check memory permission bits for half-word (u16) and emit fault if permissions don't match.
                            let reg2 = alloc_hostreg(&mut host_regs);
                            let reg2h = reg2.narrow();
                            tb.mov(reg2h, Mem16::indirect_base_index(Reg64::r9, reg1.wider()));
                            tb.and(reg2h, Imm16::from(PROT_W));
                            tb.cmp(reg2h, Imm16::from(PROT_W));
                            tb.jnz(&mut fault);

                            // Check if writing to executable memory, if so then fault. No support for potentially self-modifying code.
                            tb.mov(reg2h, Mem16::indirect_base_index(Reg64::r9, reg1.wider()));
                            tb.and(reg2h, Imm16::from(PROT_X));
                            tb.test(reg2h, reg2h);
                            tb.jnz(&mut fault);
                            free_hostreg(&mut host_regs, reg2);

                            // Load halt-word to store from guest reg.
                            let reg2 = emit_load_reg(&mut tb, &mut host_regs, rs2);
                            let reg2h = reg2.narrow();

                            // Store word in guest vmem.
                            tb.mov(Mem16::indirect_base_index(Reg64::r8, reg1.wider()), reg2h);

                            free_hostreg(&mut host_regs, reg2);
                        },
                        Insn::Sw { .. } => {
                            // Check memory permission bits for word (u32) and emit fault if permissions don't match.
                            let reg2 = alloc_hostreg(&mut host_regs);
                            tb.mov(reg2, Mem32::indirect_base_index(Reg64::r9, reg1.wider()));
                            tb.and(reg2, Imm32::splat_u8(PROT_W));
                            tb.cmp(reg2, Imm32::splat_u8(PROT_W));
                            tb.jnz(&mut fault);

                            // Check if writing to executable memory, if so then fault. No support for potentially self-modifying code.
                            tb.mov(reg2, Mem32::indirect_base_index(Reg64::r9, reg1.wider()));
                            tb.and(reg2, Imm32::splat_u8(PROT_X));
                            tb.test(reg2, reg2);
                            tb.jnz(&mut fault);
                            free_hostreg(&mut host_regs, reg2);

                            // Load word to store from guest reg.
                            let reg2 = emit_load_reg(&mut tb, &mut host_regs, rs2);

                            // Store word in guest vmem.
                            tb.mov(Mem32::indirect_base_index(Reg64::r8, reg1.wider()), reg2);

                            free_hostreg(&mut host_regs, reg2);
                        },
                        i @ _ => unreachable!("{i:?}"),
                    }

                    free_hostreg(&mut host_regs, reg1);
                }
                Insn::Addi  { rd, rs1, imm } |
                Insn::Slti  { rd, rs1, imm } |
                Insn::Sltiu { rd, rs1, imm } |
                Insn::Xori  { rd, rs1, imm } |
                Insn::Ori   { rd, rs1, imm } |
                Insn::Andi  { rd, rs1, imm } |
                Insn::Slli  { rd, rs1, shamt: imm } |
                Insn::Srli  { rd, rs1, shamt: imm } |
                Insn::Srai  { rd, rs1, shamt: imm } => {
                    let reg = emit_load_reg(&mut tb, &mut host_regs, rs1);

                    match insn {
                        Insn::Addi  { .. } => tb.add(reg, Imm32::from(imm)),
                        Insn::Slti  { .. } => {
                            tb.cmp(reg, Imm32::from(imm));
                            let regb = reg.narrow().narrow();
                            tb.setl(regb);
                            tb.movzx(reg, regb);
                        },
                        Insn::Sltiu { .. } => {
                            tb.cmp(reg, Imm32::from(imm));
                            let regb = reg.narrow().narrow();
                            tb.setb(regb);
                            tb.movzx(reg, regb);
                        },
                        Insn::Xori  { .. } => tb.xor(reg, Imm32::from(imm)),
                        Insn::Ori   { .. } => tb.or(reg, Imm32::from(imm)),
                        Insn::Andi  { .. } => tb.and(reg, Imm32::from(imm)),
                        Insn::Slli  { .. } => {
                            debug_assert!(imm < i32::from(u8::MAX));
                            tb.shl(reg, Imm8::from(imm as u8))
                        },
                        Insn::Srli  { .. } => {
                            debug_assert!(imm < i32::from(u8::MAX));
                            tb.shr(reg, Imm8::from(imm as u8))
                        },
                        Insn::Srai  { .. } => {
                            debug_assert!(imm < i32::from(u8::MAX));
                            tb.sar(reg, Imm8::from(imm as u8))
                        },
                        i @ _ => unreachable!("{i:?}"),
                    };

                    emit_store_reg(&mut tb, &mut host_regs, rd, reg);
                }
                Insn::Add  { rd, rs1, rs2 } |
                Insn::Sub  { rd, rs1, rs2 } |
                Insn::Sll  { rd, rs1, rs2 } |
                Insn::Slt  { rd, rs1, rs2 } |
                Insn::Sltu { rd, rs1, rs2 } |
                Insn::Xor  { rd, rs1, rs2 } |
                Insn::Srl  { rd, rs1, rs2 } |
                Insn::Sra  { rd, rs1, rs2 } |
                Insn::Or   { rd, rs1, rs2 } |
                Insn::And  { rd, rs1, rs2 } => {
                    let reg1 = emit_load_reg(&mut tb, &mut host_regs, rs1);
                    let reg2 = emit_load_reg(&mut tb, &mut host_regs, rs2);

                    match insn {
                        Insn::Add  { .. } => tb.add(reg1, reg2),
                        Insn::Sub  { .. } => tb.sub(reg1, reg2),
                        Insn::Sll  { .. } => {
                            // shl r32, cl is the only register variant.
                            tb.push(Reg64::rcx);
                            tb.mov(Reg8::cl, reg2.narrow().narrow());
                            tb.shl(reg1, Reg8::cl);
                            tb.pop(Reg64::rcx);
                        },
                        Insn::Slt  { .. } => {
                            tb.cmp(reg1, reg2);
                            let regb = reg1.narrow().narrow();
                            tb.setl(regb);
                            tb.movzx(reg1, regb);
                        },
                        Insn::Sltu { .. } => {
                            tb.cmp(reg1, reg2);
                            let regb = reg1.narrow().narrow();
                            tb.setb(regb);
                            tb.movzx(reg1, regb);
                        },
                        Insn::Xor  { .. } => tb.xor(reg1, reg2),
                        Insn::Srl  { .. } => {
                            // shr r32, cl is the only register variant.
                            tb.push(Reg64::rcx);
                            tb.mov(Reg8::cl, reg2.narrow().narrow());
                            tb.shr(reg1, Reg8::cl);
                            tb.pop(Reg64::rcx);
                        },
                        Insn::Sra  { .. } => {
                            // sar r32, cl is the only register variant.
                            tb.push(Reg64::rcx);
                            tb.mov(Reg8::cl, reg2.narrow().narrow());
                            tb.sar(reg1, Reg8::cl);
                            tb.pop(Reg64::rcx);
                        },
                        Insn::Or   { .. } => tb.or(reg1, reg2),
                        Insn::And  { .. } => tb.and(reg1, reg2),
                        i @ _ => unreachable!("{i:?}"),
                    };

                    emit_store_reg(&mut tb, &mut host_regs, rd, reg1);
                    free_hostreg(&mut host_regs, reg2);
                }
                Insn::Fence { .. } => {},
                Insn::Ecall =>  {
                    emit_ret_imm(&mut tb, JIT_ECALL, pc + 4);
                    break 'outer;
                }
                Insn::Ebreak =>  {
                    emit_ret_imm(&mut tb, JIT_EBREAK, pc + 4);
                    break 'outer;
                }
                Insn::Lr { rd, rs1, .. } => {
                    // Implement LR as a simple load w/o the reservation set to
                    // implement the exclusive access. Also make the blunt
                    // assumption that sw "behaves" and does not break the exclusive
                    // access on a single thread.

                    // Effective address.
                    let reg1 = emit_load_reg(&mut tb, &mut host_regs, rs1);

                    let mut check = Label::new();
                    let mut fault = Label::new();

                    // Check if effective address is out of bounds of the guest vmem.
                    tb.cmp(reg1, Imm32::from(self.vmem.len() as u32));
                    tb.jb(&mut check);

                    // Emit exit block for load faults.
                    tb.bind(&mut fault);

                    emit_ret_imm(&mut tb, JIT_LD_FAULT, pc + 4);
                    tb.bind(&mut check);

                    // Check memory permission bits for word (u32) and emit fault if permissions don't match.
                    let reg2 = alloc_hostreg(&mut host_regs);
                    tb.mov(reg2, Mem32::indirect_base_index(Reg64::r9, reg1.wider()));
                    tb.and(reg2, Imm32::splat_u8(PROT_R));
                    tb.cmp(reg2, Imm32::splat_u8(PROT_R));
                    tb.jnz(&mut fault);
                    free_hostreg(&mut host_regs, reg2);

                    // Load word from guest vmem.
                    tb.mov(reg1, Mem32::indirect_base_index(Reg64::r8, reg1.wider()));

                    // Save word in register.
                    emit_store_reg(&mut tb, &mut host_regs, rd, reg1);
                }
                Insn::Sc { rd, rs1, rs2, .. } => {
                    // Implement SC as a simple store w/o the reservation set to
                    // implement the exclusive access. Also make the blunt
                    // assumption that sw "behaves" and does not break the exclusive
                    // access on a single thread.

                    // Effective address.
                    let reg1 = emit_load_reg(&mut tb, &mut host_regs, rs1);

                    let mut check = Label::new();
                    let mut fault = Label::new();

                    // Check if effective address is out of bounds of the guest vmem.
                    tb.cmp(reg1, Imm32::from(self.vmem.len() as u32));
                    tb.jb(&mut check);

                    // Emit exit block for load faults.
                    tb.bind(&mut fault);
                    emit_ret_imm(&mut tb, JIT_ST_FAULT, pc + 4);
                    tb.bind(&mut check);

                    // Check memory permission bits for word (u32) and emit fault if permissions don't match.
                    let reg2 = alloc_hostreg(&mut host_regs);
                    tb.mov(reg2, Mem32::indirect_base_index(Reg64::r9, reg1.wider()));
                    tb.and(reg2, Imm32::splat_u8(PROT_W));
                    tb.cmp(reg2, Imm32::splat_u8(PROT_W));
                    tb.jnz(&mut fault);

                    // Check if writing to executable memory, if so then fault. No support for potentially self-modifying code.
                    tb.mov(reg2, Mem32::indirect_base_index(Reg64::r9, reg1.wider()));
                    tb.and(reg2, Imm32::splat_u8(PROT_X));
                    tb.test(reg2, reg2);
                    tb.jnz(&mut fault);
                    free_hostreg(&mut host_regs, reg2);

                    // Load word to store from guest reg.
                    let reg2 = emit_load_reg(&mut tb, &mut host_regs, rs2);

                    // Store word in guest vmem.
                    tb.mov(Mem32::indirect_base_index(Reg64::r8, reg1.wider()), reg2);

                    free_hostreg(&mut host_regs, reg2);
                    free_hostreg(&mut host_regs, reg1);

                    // Return that Sc was successful.
                    emit_store_reg_imm(&mut tb, rd, 0);
                }
            }

            // Advance to next instruction.
            pc = pc.wrapping_add(4);

            debug_assert!(host_regs.len() == 3, "Host reg leaked after INSN!");
        }
        debug_assert!(host_regs.len() == 3, "Host reg leaked after TB!");

        unsafe { self.rt.add_code::<JitFn>(tb.into_code()) }
    }
}

// -- CREATE GUEST -------------------------------------------------------------

/// Create a rv32i guest and load the `elf` file.
pub fn create_guest(elf: &[u8]) -> GuestState {
    let mut state = GuestState::new(8 * 1024 * 1024);
    match elfload::Elf::parse(elf) {
        Ok(elf) => {
            // No fs support.
            assert!(!elf.has_interp());

            println!(
                "ELF machine: {:?} entry: 0x{:08x}",
                elf.machine(),
                elf.entry(),
            );
            for l in elf.load_segments() {
                let prot = if l.read() { PROT_R } else { 0 }
                    | if l.write() { PROT_W } else { 0 }
                    | if l.exec() { PROT_X } else { 0 };
                state.map_mem(l.vaddr().try_into().unwrap(), l.bytes(), prot);

                if l.zero_padding() > 0 {
                    let addr = (l.vaddr() + l.bytes().len() as u64).try_into().unwrap();
                    state.map_mem_zero(addr, l.zero_padding().try_into().unwrap(), prot);
                }
            }
            state.pc = elf.entry().try_into().unwrap();
        }
        Err(e) => {
            panic!("Parsing ELF file failed with {:?}.", e);
        }
    };

    state
}

// -- TARGET UTILITIES ---------------------------------------------------------

mod target {
    type Usize = u32;

    #[repr(C)]
    pub struct Iovec {
        pub iov_base: Usize,
        pub iov_len: Usize,
    }

    impl TryFrom<&[u8]> for Iovec {
        type Error = ();
        fn try_from(bytes: &[u8]) -> Result<Self, Self::Error> {
            if bytes.len() < core::mem::size_of::<Self>() {
                Err(())
            } else {
                const TUSZ_SIZE: usize = core::mem::size_of::<Usize>();
                let target_usize = |idx: usize| {
                    Usize::from_le_bytes(bytes[idx..idx + TUSZ_SIZE].try_into().unwrap())
                };
                Ok(Iovec {
                    iov_base: target_usize(0),
                    iov_len: target_usize(TUSZ_SIZE),
                })
            }
        }
    }

    pub const SYS_IOCTL: u32 = 29;
    pub const SYS_WRITE: u32 = 64;
    pub const SYS_WRITEV: u32 = 66;
    pub const SYS_EXIT: u32 = 93;
    pub const SYS_EXIT_GROUP: u32 = 94;
    pub const SYS_SET_TID_ADDRESS: u32 = 96;
}

// -- SYSCALL HANDLER ----------------------------------------------------------

/// Handle a guest syscall.
fn handle_syscall(state: &mut GuestState) -> Option<()> {
    let syscall = state.read_reg(A7);
    let arg0 = state.read_reg(A0);
    let arg1 = state.read_reg(A1);
    let arg2 = state.read_reg(A2);

    match syscall {
        target::SYS_WRITE => {
            let data = state.slice_mem(arg1, arg2 as usize);
            let s = std::str::from_utf8(data).unwrap();
            trace!("syscall", "write({}, {:x}, {})", arg0, arg1, arg2);
            print!("{}", s);
            state.write_reg(A0, s.len() as u32);
        }
        target::SYS_WRITEV => {
            let mut cnt = 0;
            for v in 0..arg2 {
                let offset = v
                    .checked_mul(core::mem::size_of::<target::Iovec>() as u32)
                    .unwrap();
                let data = state.slice_mem(arg1 + offset, core::mem::size_of::<target::Iovec>());
                let iov = target::Iovec::try_from(data).unwrap();

                let data = state.slice_mem(iov.iov_base, iov.iov_len as usize);
                let s = std::str::from_utf8(data).unwrap();
                trace!("syscall", "writev({}, {:x}, {})", arg0, arg1, arg2);
                print!("{}", s);
                cnt += s.len();
            }
            state.write_reg(A0, cnt as u32);
        }
        target::SYS_EXIT => {
            trace!("syscall", "exit({})", arg0);
            return None;
        }
        target::SYS_EXIT_GROUP => {
            trace!("syscall", "exit_group({})", arg0);
            return None;
        }
        target::SYS_IOCTL | target::SYS_SET_TID_ADDRESS => {
            trace!("syscall", "syscall({}) ignored", syscall);
        }
        n @ _ => todo!("unimplemented syscall {}", n),
    }
    Some(())
}

// -- MAIN ---------------------------------------------------------------------

fn main() {
    // Parse command line args.
    let (elf_name, elf) = match std::env::args().skip(1).next() {
        Some(s) if s == "guest1" => (b"guest1\0", include_bytes!("rv32i-guest/guest1").as_slice()),
        Some(s) if s == "guest2" => (b"guest2\0", include_bytes!("rv32i-guest/guest2").as_slice()),
        None => (b"guest1\0", include_bytes!("rv32i-guest/guest1").as_slice()),
        Some(s) => panic!("Unsupported program name '{}'!", s),
    };

    // Create guest and load elf file into guest virtual memory.
    let mut state = create_guest(elf);

    // Map and create an initial stack.
    let sp = {
        // Map a zeroed out stack.
        const STACK_SIZE: u32 = 8 * 4096;
        let mut sp: u32 = state.vmem.len().try_into().unwrap();
        state.map_mem_zero(sp - STACK_SIZE, STACK_SIZE as usize, PROT_R | PROT_W);

        // Move the stack pointer down and push the bytes onto the stack. Return
        // the stack pointer to the beginning of the pushed data.
        macro_rules! push_bytes {
            ($bytes:expr) => {{
                let len: u32 = $bytes.len().try_into().expect("must fit into u32");
                sp -= len;
                state.write_mem(sp, $bytes);
                sp
            }};
        }

        // Move the stack pointer down and push the value as u32 onto the
        // stack. Return the stack pointer to the beginning of the pushed data.
        macro_rules! push_ptr {
            ($ptr:expr) => {{
                let ptr_val = $ptr as u32;
                push_bytes!(&ptr_val.to_le_bytes());
            }};
        }

        // Create the following process image on the stack as defined by the
        // SystemV abi.
        //
        //         +------------+ High Address
        //         | ..         |
        //         | ENV strs   |<-+
        //      +->| ARG strs   |  |
        //      |  | ..         |  |
        //      |  +------------+  |
        //      |  | ..         |  |
        //      |  +------------+  |
        //      |  | AT_NULL    |  |
        //      |  +------------+  |
        //      |  | AUXV       |  |
        //      |  +------------+  |
        //      |  | 0x0        |  |
        //      |  +------------+  |
        //      |  | ENVP       |--+
        //      |  +------------+
        //      |  | 0x0        |
        //      |  +------------+
        //      +--| ARGV       |
        //         +------------+
        //  $rsp ->| ARGC       |
        //        +------------+ Low Address

        // Push actual argv strings.
        let arg0 = push_bytes!(elf_name);
        let arg1 = push_bytes!(b"moose\0");
        let arg2 = push_bytes!(b"elk\0");

        push_ptr!(0u32); // auxv | AT_NULL val
        push_ptr!(0u32); // auxv | AT_NULL tag
        push_ptr!(0u32); // envp null terminator
        push_ptr!(0u32); // argv null terminator
        push_ptr!(arg2); // argv[2]
        push_ptr!(arg1); // argv[1]
        push_ptr!(arg0); // argv[0]
        push_ptr!(3u32); // argc

        sp
    };

    // Initialize stack pointer register.
    state.write_reg(SP, sp);

    // Run the guest, and toggle jit and interpreter mode after each VM exit.
    let mut is_jit = true;
    'outer: loop {
        let ret = if is_jit {
            state.jit()
        } else {
            state.interpret()
        };
        is_jit = !is_jit;

        match ret {
            ExitReason::Ecall => {
                if matches!(handle_syscall(&mut state), None) {
                    break 'outer;
                }
            }
            r @ _ => todo!("unhandled exit reason {:?}", r),
        }
    }
}
