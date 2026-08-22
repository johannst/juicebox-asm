// SPDX-License-Identifier: MIT
//
// Copyright (c) 2026, Johannes Stoelp <dev@memzero.de>

//! RISC-V 32bit.
//!
//! This example implements a minimal rv32i userspace emulator with an
//! interpreter and jit compiler to demonstrate the juicebox crate.
//!
//! The emulator only implements a very limited syscall surface, sufficient to
//! run the example software in examples/rv32i-guest/. However, enough to run a
//! multi-threaded example with TLS support.
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
use std::str::FromStr;

use juicebox_asm::insn::*;
use juicebox_asm::Runtime;
use juicebox_asm::{Asm, Imm16, Imm32, Imm64, Imm8, Label, Mem16, Mem32, Mem8, Reg32, Reg64, Reg8};

// Enable tracing of different parts of the emulator (mainly for debugging).
const ENABLE_TRACE: bool = false;
const TRACE_FILTER: &'static str = "syscall";

macro_rules! trace {
    ($tag:expr, $($arg:tt)*) => ({
        if ENABLE_TRACE && (TRACE_FILTER == "all" || TRACE_FILTER == $tag) {
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
/// Register ABI name - Thread Pointer.
pub const TP: RegIdx = 4;
/// Register ABI name - Argument 0.
pub const A0: RegIdx = 10;
/// Register ABI name - Argument 1.
pub const A1: RegIdx = 11;
/// Register ABI name - Argument 2.
pub const A2: RegIdx = 12;
/// Register ABI name - Argument 3.
pub const A3: RegIdx = 13;
/// Register ABI name - Argument 4.
pub const A4: RegIdx = 14;
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
            rt: Runtime::with_capacity(64),
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
            Insn::Fence {..} => {},
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
                    trace!("jit", "{:08x} {:?}", pc, insn);
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

// -- GUEST UTILS --------------------------------------------------------------

pub struct PhdrInfo {
    phaddr: u32,
    phentsize: u32,
    phnum: u32,
}

/// Load the `elf` file into the `guest` and return the entry point and
/// information about the program header.
pub fn load_elf(guest: &mut GuestState, elf: &[u8]) -> (u32, PhdrInfo) {
    let mut phdr = None;
    let entry = match elfload::Elf::parse(elf) {
        Ok(elf) => {
            // No fs support.
            assert!(!elf.has_interp());

            println!(
                "ELF machine: {:?} entry: 0x{:08x}",
                elf.machine(),
                elf.entry(),
            );

            for seg in elf.segments() {
                match seg.typ() {
                    elfload::SegmentType::Load => {
                        let prot = if seg.read() { PROT_R } else { 0 }
                            | if seg.write() { PROT_W } else { 0 }
                            | if seg.exec() { PROT_X } else { 0 };
                        guest.map_mem(seg.vaddr().try_into().unwrap(), seg.bytes(), prot);

                        if seg.zero_padding() > 0 {
                            let addr = (seg.vaddr() + seg.bytes().len() as u64).try_into().unwrap();
                            guest.map_mem_zero(addr, seg.zero_padding().try_into().unwrap(), prot);
                        }
                    }
                    elfload::SegmentType::Phdr => {
                        phdr = Some(PhdrInfo {
                            phaddr: seg.vaddr().try_into().unwrap(),
                            phentsize: elf.phentsize().try_into().unwrap(),
                            phnum: elf.phnum().try_into().unwrap(),
                        });
                    }
                    _ => {}
                }
            }

            elf.entry()
        }
        Err(e) => {
            panic!("Parsing ELF file failed with {:?}.", e);
        }
    };

    (entry.try_into().unwrap(), phdr.expect("Must have PT_PHDR"))
}

/// Map zeroed out stack of `size` bytes at the end of the guest address space
/// and return (final stack pointer, stack bottom). The stack is initialized
/// according to the SystemV abi (Process Initialization - Stack State).
///
/// https://github.com/johannst/dynld/tree/main/02_process_init
/// https://github.com/torvalds/linux/blob/c84d3e3130dfe1058cb27dc78e7ad8bd36f0545a/fs/binfmt_elf.c#L165
pub fn setup_main_stack(
    guest: &mut GuestState,
    size: u32,
    prog: &[u8],
    phdr: PhdrInfo,
) -> (u32, u32) {
    // Map a zeroed out stack at the end of the guest memory.
    let mut sp: u32 = guest.vmem.len().try_into().unwrap();
    let stack_bottom = sp - size;
    guest.map_mem_zero(stack_bottom, size as usize, PROT_R | PROT_W);

    // Move the stack pointer down and push the bytes onto the stack. Return
    // the stack pointer to the beginning of the pushed data.
    macro_rules! push_bytes {
        ($bytes:expr) => {{
            let len: u32 = $bytes.len().try_into().expect("must fit into u32");
            sp -= len;
            guest.write_mem(sp, $bytes);
            sp
        }};
    }

    // Move the stack pointer down and push the value as u32 onto the
    // stack. Return the stack pointer to the beginning of the pushed data.
    macro_rules! push_ptr {
        ($ptr:expr) => {{
            let ptr = $ptr as u32;
            push_bytes!(&ptr.to_le_bytes());
        }};
    }

    // Move the stack pointer down and push the auxvec tag and value as u32 onto
    // the stack. Return the stack pointer to the beginning of the pushed data.
    macro_rules! push_aux {
        ($tag:expr, $val:expr) => {{
            let tag = $tag as u32;
            let val = $val as u32;
            push_bytes!(&val.to_le_bytes());
            push_bytes!(&tag.to_le_bytes());
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
    let arg0 = push_bytes!(prog);
    let arg1 = push_bytes!(b"moose\0");
    let arg2 = push_bytes!(b"elk\0");

    const AT_NULL: u32 = 0;
    const AT_PAGESZ: u32 = 6;
    const AT_PHNUM: u32 = 5;
    const AT_PHENT: u32 = 4;
    const AT_PHDR: u32 = 3;

    push_aux!(AT_NULL, 0u32);
    push_aux!(AT_PAGESZ, target::PAGE_SIZE);
    push_aux!(AT_PHNUM, phdr.phnum); // for TLS support, to find the initial TLS image
    push_aux!(AT_PHENT, phdr.phentsize); // for TLS support, to find the initial TLS image
    push_aux!(AT_PHDR, phdr.phaddr); // for TLS supportm  to find the initial TLS image

    push_ptr!(0u32); // envp null terminator
    push_ptr!(0u32); // argv null terminator
    push_ptr!(arg2); // argv[2]
    push_ptr!(arg1); // argv[1]
    push_ptr!(arg0); // argv[0]
    push_ptr!(3u32); // argc

    (sp, stack_bottom)
}

// -- TARGET UTILITIES ---------------------------------------------------------

mod target {
    type Usize = u32;

    pub const PAGE_SIZE: u32 = 4096;
    pub const STACK_SIZE: u32 = 8 * PAGE_SIZE;

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
    pub const SYS_RT_SIGPROCMASK: u32 = 135;
    pub const SYS_MUNMAP: u32 = 215;
    pub const SYS_CLONE: u32 = 220;
    pub const SYS_MMAP: u32 = 222;
    pub const SYS_MPROTECT: u32 = 226;
    pub const SYS_FUTEX: u32 = 422;
}

// -- SYSCALL HANDLER ----------------------------------------------------------

/// Guest thread states.
#[derive(Clone, Copy)]
pub enum ThreadState {
    /// Thread is in running state and can be scheduled.
    Running,

    /// Thread is blocked in a `FUTEX_WAIT` operation on the address.
    FutexWait(u32),

    /// Thread has exited with the exit status.
    Exit(u32),
}

/// Guest thread context that can be saved and restored to achieve scheduling
/// multiple guest threads on a single guest vcpu.
pub struct ThreadCtx {
    /// The threads guest register state.
    regs: [u32; 32],

    /// The threads guest PC.
    pc: u32,

    /// The threads current state.
    state: ThreadState,

    /// The thread id.
    tid: Option<u32>,

    /// Guest address to the CLEARTID location. If not 0, the thread zeros the
    /// memory word and does a `FUTEX_WAKE` operation on that address when it
    /// exits.
    cleartid: u32,
}

impl ThreadCtx {
    /// Create a new thread context which is in running state.
    pub fn new() -> ThreadCtx {
        ThreadCtx {
            regs: [0; 32],
            pc: 0,
            state: ThreadState::Running,
            tid: None,
            cleartid: 0,
        }
    }

    /// Update guest register in the thread context.
    pub fn write_reg(&mut self, r: RegIdx, val: u32) {
        if r == 0 {
            return;
        }

        let idx = r as usize;
        debug_assert!(idx < self.regs.len());
        self.regs[idx] = val;
    }

    /// Save the current guest state in this thread context.
    pub fn save(&mut self, guest: &GuestState) {
        self.regs = guest.regs;
        self.pc = match guest.reenter_pc {
            Some(pc) => pc,
            None => guest.pc,
        };
    }

    /// Restore the current thread context in the guest state.
    pub fn restore(&self, guest: &mut GuestState) {
        guest.regs = self.regs;
        guest.pc = self.pc;
        guest.reenter_pc = None;
    }
}

/// A process context which serves as container for multiple threads and
/// maintains shared information.
pub struct ProcessCtx {
    /// Available threads in the process.
    threads: Vec<ThreadCtx>,

    /// Index of the currently active thread.
    current: usize,

    /// Guest address to the next free mmap area.
    mmap_ptr: u32,

    /// Guest address to the end of the available mmap area.
    mmap_end: u32,

    /// Next TID to allocated for a new thread.
    next_tid: u32,
}

impl ProcessCtx {
    /// Find next runnable thread and return the index into threads vector if found.
    pub fn find_next_runnable(&self) -> Option<usize> {
        // Offset to start searching from.
        let off = self.current + 1;
        // Find next runnable thread and adjust index according to the start offset.
        self.threads[off..]
            .iter()
            .chain(self.threads[..off].iter())
            .position(|th| matches!(th.state, ThreadState::Running))
            .map(|idx| (idx + off) % self.threads.len())
    }
}

/// Handle a guest syscall.
fn handle_syscall(guest: &mut GuestState, proc: &mut ProcessCtx) -> ThreadState {
    let syscall = guest.read_reg(A7);
    let arg0 = guest.read_reg(A0);
    let arg1 = guest.read_reg(A1);
    let arg2 = guest.read_reg(A2);
    let arg3 = guest.read_reg(A3);
    let arg4 = guest.read_reg(A4);

    let tid = proc.threads[proc.current]
        .tid
        .expect("active thread has TID");

    let futex_wake = |proc: &mut ProcessCtx, uaddr: u32| {
        for th in &mut proc.threads {
            if matches!(th.state, ThreadState::FutexWait(ua) if ua == uaddr) {
                th.state = ThreadState::Running;
            }
        }
    };

    use target::*;
    let (ret, state) = match syscall {
        SYS_WRITE => {
            trace!(
                "syscall",
                "[{}] write(fd={}, addr=0x{:x}, len={})",
                tid,
                arg0,
                arg1,
                arg2
            );
            let _fd = arg0;
            let addr = arg1;
            let len = arg2 as usize;
            let data = guest.slice_mem(addr, len);
            let s = std::str::from_utf8(data).unwrap();
            print!("{}", s);
            (s.len() as u32, ThreadState::Running)
        }
        SYS_WRITEV => {
            trace!(
                "syscall",
                "[{}] writev(fd={}, iov=0x{:x}, iov_cnt={})",
                tid,
                arg0,
                arg1,
                arg2
            );
            let mut cnt = 0;
            let iov_addr = arg1;
            let iov_cnt = arg2;
            for v in 0..iov_cnt {
                let offset = v.checked_mul(core::mem::size_of::<Iovec>() as u32).unwrap();
                let data = guest.slice_mem(iov_addr + offset, core::mem::size_of::<Iovec>());
                let iov = Iovec::try_from(data).unwrap();

                let data = guest.slice_mem(iov.iov_base, iov.iov_len as usize);
                let s = std::str::from_utf8(data).unwrap();
                print!("{}", s);
                cnt += s.len();
            }
            (cnt as u32, ThreadState::Running)
        }
        SYS_EXIT => {
            trace!("syscall", "[{}] exit({})", tid, arg0);
            let status = arg0;

            // If set, zero out the CLEARTID locationd wake any futex waiters.
            let cleartid = proc.threads[proc.current].cleartid;
            if cleartid != 0 {
                mem_write!(guest, u32, cleartid, 0);
                futex_wake(proc, cleartid);
            }

            (0, ThreadState::Exit(status))
        }
        SYS_EXIT_GROUP => {
            trace!("syscall", "[{}] exit_group({})", tid, arg0);
            let status = arg0;
            (0, ThreadState::Exit(status))
        }
        SYS_SET_TID_ADDRESS => {
            trace!("syscall", "[{}] set_tid_address({:08x})", tid, arg0);
            let tidptr = arg0;
            proc.threads[proc.current].cleartid = tidptr;
            (0, ThreadState::Running)
        }
        SYS_CLONE => {
            trace!(
                "syscall", "[{}] clone(flags=0x{:x}, sp=0x{:08x}, parent_tidptr=0x{:08x}, tls=0x{:08x}, child_tidptr=0x{:08x})",
                tid, arg0, arg1, arg2, arg3, arg4
            );
            let flags = arg0;
            let sp = arg1;
            let ptidptr = arg2;
            let tlsp = arg3;
            let ctidptr = arg4;

            const CLONE_VM: u32 = 0x100;
            const CLONE_THREAD: u32 = 0x1_0000;
            const CLONE_SETTLS: u32 = 0x8_0000;
            const CLONE_PARENT_SETTID: u32 = 0x10_0000;
            const CLONE_CHILD_CLEARTID: u32 = 0x20_0000;
            const CLONE_CHILD_SETTID: u32 = 0x100_0000;

            assert!(flags & CLONE_VM != 0);
            assert!(flags & CLONE_THREAD != 0);
            assert!(sp > 0);

            let mut child_ctx = ThreadCtx::new();
            child_ctx.save(guest);

            // Set guest return value.
            child_ctx.regs[A0 as usize] = 0;

            // Set guest stack pointer.
            child_ctx.regs[SP as usize] = sp;

            // Set guest thread pointer.
            if flags & CLONE_SETTLS != 0 {
                child_ctx.regs[TP as usize] = tlsp;
            }

            // Set guest thread id.
            let tid = proc.next_tid;
            proc.next_tid += 1;
            child_ctx.tid = Some(tid);
            // Write guest tid into parent's tid location.
            if flags & CLONE_PARENT_SETTID != 0 {
                mem_write!(guest, u32, ptidptr, tid);
            }
            // Write guest tid into child's tid location.
            if flags & CLONE_CHILD_SETTID != 0 {
                mem_write!(guest, u32, ctidptr, tid);
            }
            // Store clear tid location, which is zeroed out and futex wake'ed
            // when the thread exits.
            if flags & CLONE_CHILD_CLEARTID != 0 {
                child_ctx.cleartid = ctidptr;
            }

            proc.threads.push(child_ctx);

            (tid, ThreadState::Running)
        }
        SYS_MMAP => {
            trace!(
                "syscall",
                "[{}] mmap(addr=0x{:x}, len=0x{:x}, prot=0x{:x}, ...)",
                tid,
                arg0,
                arg1,
                arg2
            );

            assert!(arg0 == 0); // no fixed addr
            assert!(arg1 % PAGE_SIZE == 0);
            let len = arg1;
            let prot = arg2 as u8;

            const MAP_FAILED: u32 = 0xffff_ffff;
            let ret = if proc.mmap_ptr + len > proc.mmap_end {
                MAP_FAILED
            } else {
                let addr = proc.mmap_ptr;
                proc.mmap_ptr += len;
                guest.map_mem_zero(addr, len as usize, prot);
                addr
            };
            (ret, ThreadState::Running)
        }
        SYS_MUNMAP => {
            trace!(
                "syscall",
                "[{}] munmap(addr=0x{:x}, len=0x{:x})",
                tid,
                arg0,
                arg1
            );

            assert!(arg0 % PAGE_SIZE == 0);
            assert!(arg1 % PAGE_SIZE == 0);
            let addr = arg0;
            let len = arg1 as usize;

            guest.set_prot(addr, len, 0);
            (0, ThreadState::Running)
        }
        SYS_MPROTECT => {
            trace!(
                "syscall",
                "[{}] mprotect(addr=0x{:08x}, len=0x{:x}, prot=0x{:x})",
                tid,
                arg0,
                arg1,
                arg2
            );
            let addr = arg0;
            let len = arg1 as usize;
            let prot = arg2 as u8;
            guest.set_prot(addr, len, prot);
            (0, ThreadState::Running)
        }
        SYS_FUTEX => {
            trace!(
                "syscall",
                "[{}] futex(addr=0x{:08x}, op=0x{:x}, ..)",
                tid,
                arg0,
                arg1
            );

            const FUTEX_WAIT: u32 = 0;
            const FUTEX_WAKE: u32 = 1;
            const FUTEX_CMD_MASK: u32 = 0x7f;

            let uaddr = arg0;
            let cmd = arg1 & FUTEX_CMD_MASK;

            match cmd {
                FUTEX_WAIT => (0, ThreadState::FutexWait(uaddr)),
                FUTEX_WAKE => {
                    futex_wake(proc, uaddr);
                    (0, ThreadState::Running)
                }
                _ => todo!("unhandled futex cmd 0x{:x}", cmd),
            }
        }
        SYS_IOCTL | SYS_RT_SIGPROCMASK => {
            trace!("syscall", "[{}] syscall({}) ignored", tid, syscall);
            (0, ThreadState::Running)
        }
        n @ _ => todo!("unimplemented syscall {}", n),
    };

    guest.write_reg(A0, ret);
    state
}

// -- MAIN ---------------------------------------------------------------------

fn main() {
    // Parse command line args.
    let (elf_name, elf) = match std::env::args().skip(1).next() {
        Some(s) if std::fs::exists(&s).is_ok() => {
            (std::ffi::CString::from_str(&s), std::fs::read(&s))
        }
        Some(s) => panic!("Guest program not found '{}'!", s),
        None => panic!("Provide guest program as first argument!"),
    };
    let elf = elf.expect("guest elf file not found");
    let elf_name = elf_name.expect("guest elf path could not be converted to c str");

    // Create guest and load elf file into guest virtual memory.
    let mut guest = GuestState::new(8 * 1024 * 1024);
    let (entry, phdr) = load_elf(&mut guest, &elf);

    // Create and initialize the stack for the main thread.
    let (sp, stack_bottom) = setup_main_stack(
        &mut guest,
        target::STACK_SIZE,
        elf_name.as_bytes_with_nul(),
        phdr,
    );

    // Initialize a thread context for the main thread.
    let mut main_thread = ThreadCtx::new();
    main_thread.write_reg(SP, sp);
    main_thread.pc = entry;
    main_thread.tid = Some(1);

    // Initialize process context.
    let mut proc = {
        const MMAP_PAGES: u32 = 1024;
        let mmap_end: u32 = stack_bottom - target::PAGE_SIZE /* keep one guard page*/;
        let mmap_ptr: u32 = mmap_end - MMAP_PAGES * target::PAGE_SIZE;
        ProcessCtx {
            threads: vec![main_thread],
            current: usize::MAX,
            mmap_ptr,
            mmap_end,
            next_tid: 2,
        }
    };

    // Run the guest, and toggle jit and interpreter mode after each VM exit.
    let mut is_jit: bool = true;

    // Track the next schedulee.
    let mut next = 0;
    'outer: loop {
        assert!(matches!(proc.threads[next].state, ThreadState::Running));

        // Restore thread context on thread switch.
        if proc.current != next {
            proc.current = next;
            proc.threads[proc.current].restore(&mut guest);
        }

        let ret = if is_jit {
            guest.jit()
        } else {
            guest.interpret()
        };
        is_jit = !is_jit;

        match ret {
            ExitReason::Ecall => {
                proc.threads[proc.current].state = handle_syscall(&mut guest, &mut proc);

                match proc.threads[proc.current].state {
                    ThreadState::Running => {}
                    ThreadState::FutexWait(_) => {
                        // Thread is blocked, schedule.
                        next = proc
                            .find_next_runnable()
                            .expect("deadlock, no runnable thread");
                        assert!(next != proc.current);
                    }
                    ThreadState::Exit(_) => {
                        // Thread is exited, schedule.
                        next = match proc.find_next_runnable() {
                            Some(n) => n,
                            None => break 'outer,
                        };
                    }
                };
            }
            r @ _ => todo!("unhandled exit reason {:?}", r),
        }

        // Save thread context on thread switch.
        if proc.current != next {
            proc.threads[proc.current].save(&guest);
        }
    }

    let all_exited = proc
        .threads
        .iter()
        .all(|th| matches!(th.state, ThreadState::Exit(_)));
    assert!(all_exited, "Not all threads exited!")
}
