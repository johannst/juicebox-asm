// SPDX-License-Identifier: MIT
//
// Copyright (c) 2026, Johannes Stoelp <dev@memzero.de>

use std::convert::TryFrom;

// -- DECODER ------------------------------------------------------------------

type RegIdx = u32;

#[derive(Debug)]
struct Rtype {
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
        Rtype {
            rd: rd,
            rs1: rs1,
            rs2: rs2,
            func3,
            func7,
        }
    }
}

#[derive(Debug)]
struct Itype {
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

        Itype {
            rd: rd,
            rs1: rs1,
            func3,
            imm,
        }
    }
}

#[derive(Debug)]
struct Stype {
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

        Stype {
            rs1: rs1,
            rs2: rs2,
            func3,
            imm,
        }
    }
}

#[derive(Debug)]
struct Btype {
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

        Btype {
            rs1: rs1,
            rs2: rs2,
            func3,
            imm,
        }
    }
}

#[derive(Debug)]
struct Utype {
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

        Utype {
            rd: rd,
            imm,
        }
    }
}

#[derive(Debug)]
struct Jtype {
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

        Jtype {
            rd: rd,
            imm,
        }
    }
}

#[rustfmt::skip]
#[derive(Debug)]
#[allow(dead_code)]
 enum Insn {
    Lui   { rd: RegIdx, imm: i32 },
    Auipc { rd: RegIdx, imm: i32 },

    Jal  { rd: RegIdx, imm: i32 },

    Jalr { rd: RegIdx, rs1: RegIdx, imm: i32 },

    Beq  { rs1: RegIdx, rs2: RegIdx, imm: i32 },
    Bne  { rs1: RegIdx, rs2: RegIdx, imm: i32 },
    Blt  { rs1: RegIdx, rs2: RegIdx, imm: i32 },
    Bge  { rs1: RegIdx, rs2: RegIdx, imm: i32 },
    Bltu { rs1: RegIdx, rs2: RegIdx, imm: i32 },
    Bgeu { rs1: RegIdx, rs2: RegIdx, imm: i32 },

    Lb  { rd: RegIdx, rs1: RegIdx, imm: i32 },
    Lh  { rd: RegIdx, rs1: RegIdx, imm: i32 },
    Lw  { rd: RegIdx, rs1: RegIdx, imm: i32 },
    Lbu { rd: RegIdx, rs1: RegIdx, imm: i32 },
    Lhu { rd: RegIdx, rs1: RegIdx, imm: i32 },

    Sb { rs1: RegIdx, rs2: RegIdx, imm: i32 },
    Sh { rs1: RegIdx, rs2: RegIdx, imm: i32 },
    Sw { rs1: RegIdx, rs2: RegIdx, imm: i32 },

    Addi  { rd: RegIdx, rs1: RegIdx, imm: i32 },
    Slti  { rd: RegIdx, rs1: RegIdx, imm: i32 },
    Sltiu { rd: RegIdx, rs1: RegIdx, imm: i32 },
    Xori  { rd: RegIdx, rs1: RegIdx, imm: i32 },
    Ori   { rd: RegIdx, rs1: RegIdx, imm: i32 },
    Andi  { rd: RegIdx, rs1: RegIdx, imm: i32 },

    Slli { rd: RegIdx, rs1: RegIdx, shamt: i32 },
    Srli { rd: RegIdx, rs1: RegIdx, shamt: i32 },
    Srai { rd: RegIdx, rs1: RegIdx, shamt: i32 },

    Add  { rd: RegIdx, rs1: RegIdx, rs2: RegIdx },
    Sub  { rd: RegIdx, rs1: RegIdx, rs2: RegIdx },
    Sll  { rd: RegIdx, rs1: RegIdx, rs2: RegIdx },
    Slt  { rd: RegIdx, rs1: RegIdx, rs2: RegIdx },
    Sltu { rd: RegIdx, rs1: RegIdx, rs2: RegIdx },
    Xor  { rd: RegIdx, rs1: RegIdx, rs2: RegIdx },
    Srl  { rd: RegIdx, rs1: RegIdx, rs2: RegIdx },
    Sra  { rd: RegIdx, rs1: RegIdx, rs2: RegIdx },
    Or   { rd: RegIdx, rs1: RegIdx, rs2: RegIdx },
    And  { rd: RegIdx, rs1: RegIdx, rs2: RegIdx },

    Fence { rd: RegIdx, rs1: RegIdx, succ: i32, pred: i32, fm: i32 },

    Ecall,
    Ebreak,
}

/// Decode the riscv instruction bytes `insn`.
#[rustfmt::skip]
 fn decode(insn: u32) -> Insn {
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
        _ => todo!("instruction=0x{:08x} op=0b{:07b}", insn, opcode),
    }
}

// -- STATE --------------------------------------------------------------------

/// Register ABI name - Stack Pointer.
const SP: RegIdx = 2;
/// Register ABI name - Argument 0.
const A0: RegIdx = 10;
/// Register ABI name - Argument 1.
const A1: RegIdx = 11;
/// Register ABI name - Argument 2.
const A2: RegIdx = 12;
/// Register ABI name - Argument 7.
const A7: RegIdx = 17;

/// Protection flag - read access.
const PROT_R: u8 = 1 << 0;
/// Protection flag - write access.
const PROT_W: u8 = 1 << 1;
/// Protection flag - execute access.
const PROT_X: u8 = 1 << 2;

#[derive(Debug)]
enum ExitReason {
    /// Exit when the guest is about to execute an `ecall` instruction.
    /// The pc of the guest on exit points to the `ecall` instruction.
    Ecall,

    /// Exit when the guest is about to execute an `ebreak` instruction.
    /// The pc of the guest on exit points to the `ebreak` instruction.
    Ebreak,
}

struct GuestState {
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
}

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
    /// Create a new guest state with the virtual address space `0..mem_size-1`.
    /// The virtual address space is initially unmapped, and accessing it from
    /// the guest will raise a fault.
    fn new(mem_size: usize) -> Self {
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
        }
    }

    fn read_reg(&self, r: RegIdx) -> u32 {
        if r == 0 {
            return 0;
        }

        let idx = r as usize;
        debug_assert!(idx < self.regs.len());
        self.regs[idx]
    }

    fn write_reg(&mut self, r: RegIdx, val: u32) {
        if r == 0 {
            return;
        }

        let idx = r as usize;
        debug_assert!(idx < self.regs.len());
        self.regs[idx] = val;
    }

    /// Check if the address range `addr..addr+len` has at least the `prot`
    /// protection flag set.
    fn check_prot(&self, addr: u32, len: usize, prot: u8) -> bool {
        let start = addr as usize;
        let end = start.checked_add(len).unwrap();
        self.prot
            .get(start..end)
            .unwrap()
            .iter()
            .any(|p| (p & prot) != prot)
    }

    /// Set the `prot` protection flag for the address range `addr..addr+len`.
    /// This will just overwrite the current protection flags, and does not
    /// check if there were other protection flags set.
    fn set_prot(&mut self, addr: u32, len: usize, prot: u8) {
        let start = addr as usize;
        let end = start.checked_add(len).unwrap();
        self.prot
            .get_mut(start..end)
            .unwrap()
            .iter_mut()
            .for_each(|p| *p = prot);
    }

    /// Map the virtual address range `addr..addr+data.len()` with the
    /// protection provided in `prot` and initialize the memory with `data`.
    fn map_mem(&mut self, addr: u32, data: &[u8], prot: u8) {
        let start = addr as usize;
        let end = start.checked_add(data.len()).unwrap();
        self.set_prot(addr, data.len(), prot);
        self.vmem.get_mut(start..end).unwrap().copy_from_slice(data);

        println!(
            "MAP DATA: vaddr: 0x{:08x} len: {:4} {}{}{}",
            addr,
            data.len(),
            if prot & PROT_X != 0 { 'X' } else { '-' },
            if prot & PROT_W != 0 { 'W' } else { '-' },
            if prot & PROT_R != 0 { 'R' } else { '-' },
        );
    }

    /// Map the virtual address range `addr..addr+len` with the protection
    /// provided in `prot` and initialize the memory with `0`.
    fn map_zero_mem(&mut self, addr: u32, len: usize, prot: u8) {
        let start = addr as usize;
        let end = start.checked_add(len).unwrap();
        self.set_prot(addr, len, prot);
        self.vmem.get_mut(start..end).unwrap().fill(0);

        println!(
            "MAP ZERO: vaddr: 0x{:08x} len: {:4} {}{}{}",
            addr,
            len,
            if prot & PROT_X != 0 { 'X' } else { '-' },
            if prot & PROT_W != 0 { 'W' } else { '-' },
            if prot & PROT_R != 0 { 'R' } else { '-' },
        );
    }

    /// Read from the virtual address range `addr..addr+data.len()` into
    /// `data`. This performs a check if the address range has the read
    /// protection set.
    fn read_mem(&self, addr: u32, data: &mut [u8]) {
        let start = addr as usize;
        let end = start.checked_add(data.len()).unwrap();

        let r_fault = self.check_prot(addr, data.len(), PROT_R);
        assert!(!r_fault, "read_fault @0x{:08x} len={}", addr, data.len());

        data.copy_from_slice(&self.vmem.get(start..end).unwrap());
    }

    /// Write `data` to the virtual address range `addr..addr+data.len()`.
    /// This performs a check if the address range has the write protection set.
    fn write_mem(&mut self, addr: u32, data: &[u8]) {
        let start = addr as usize;
        let end = start.checked_add(data.len()).unwrap();

        let w_fault = self.check_prot(addr, data.len(), PROT_W);
        assert!(!w_fault, "write_fault @0x{:08x} len={}", addr, data.len());

        self.vmem.get_mut(start..end).unwrap().copy_from_slice(data);
    }

    /// Get a slice for the virtual address range `addr..addr+len`. This
    /// performs a check if the address range has the read protection set.
    fn slice_mem(&self, addr: u32, len: usize) -> &[u8] {
        let start = addr as usize;
        let end = start.checked_add(len).unwrap();

        let r_fault = self.check_prot(addr, len, PROT_R);
        assert!(!r_fault, "read_fault @0x{:08x} len={}", addr, len);

        &self.vmem.get(start..end).unwrap()
    }

    /// Fetch an instruction from `pc`. This performs a check if the address
    /// range has the exec protection set.
    fn fetch_insn(&self, pc: u32) -> u32 {
        debug_assert_eq!(pc & 0b11, 0, "PC must be 4byte aligned!");

        let x_fault = self.check_prot(pc, 4, PROT_X);
        assert!(!x_fault, "exec_fault @0x{:08x} len={}", pc, 4);

        mem_read!(self, u32, pc)
    }

    fn interpret(&mut self) -> ExitReason {
        if let Some(pc) = self.reenter_pc.take() {
            self.pc = pc;
        }

        loop {
            // Decode current instruction.
            let insn = self.fetch_insn(self.pc);
            let insn = decode(insn);

            // Interpret current instruction.
            let exit = self.step(&insn);

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

    #[rustfmt::skip]
    fn step(&mut self, insn: &Insn) -> Result<Option<u32>, ExitReason> {
        match *insn {
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
            Insn::Fence { .. } => todo!("fence"),
            Insn::Ecall =>  return Err(ExitReason::Ecall),
            Insn::Ebreak => return Err(ExitReason::Ebreak),
        }

        Ok(None)
    }
}

// -- CREATE GUEST -------------------------------------------------------------

fn create_guest(elf: &[u8]) -> GuestState {
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
                    state.map_zero_mem(addr, l.zero_padding().try_into().unwrap(), prot);
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
                Ok(Iovec {
                    iov_base: Usize::from_le_bytes(bytes[0..TUSZ_SIZE].try_into().unwrap()),
                    iov_len: Usize::from_le_bytes(
                        bytes[TUSZ_SIZE..2 * TUSZ_SIZE].try_into().unwrap(),
                    ),
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

fn handle_syscall(state: &mut GuestState) -> Option<()> {
    let syscall = state.read_reg(A7);
    let arg0 = state.read_reg(A0);
    let arg1 = state.read_reg(A1);
    let arg2 = state.read_reg(A2);

    match syscall {
        target::SYS_WRITE => {
            let data = state.slice_mem(arg1, arg2 as usize);
            let s = std::str::from_utf8(data).unwrap();
            println!("write({}, {:x}, {}) -> {}", arg0, arg1, arg2, s);
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
                println!("write({}, {:x}, {}) -> {}", arg0, arg1, arg2, s);
                cnt += s.len();
            }
            state.write_reg(A0, cnt as u32);
        }
        target::SYS_EXIT => {
            println!("exit({})", arg0);
            return None;
        }
        target::SYS_EXIT_GROUP => {
            println!("exit_group({})", arg0);
            return None;
        }
        target::SYS_IOCTL | target::SYS_SET_TID_ADDRESS => {
            println!("syscall({}) ignored", syscall);
        }
        n @ _ => todo!("unimplemented syscall {}", n),
    }
    Some(())
}

// -- MAIN ---------------------------------------------------------------------

fn main() {
    // Parse command line args.
    let elf = match std::env::args().skip(1).next() {
        Some(s) if s == "guest1" => include_bytes!("rv32i-guest/guest1").as_slice(),
        Some(s) if s == "guest2" => include_bytes!("rv32i-guest/guest2").as_slice(),
        None => include_bytes!("rv32i-guest/guest1").as_slice(),
        Some(s) => panic!("Unsupported program name '{}'!", s),
    };

    let mut state = create_guest(elf);
    let sp = {
        let sp: u32 = state.vmem.len().try_into().unwrap();
        let top: u32 = sp - 4096;
        state.map_zero_mem(top, 4096, PROT_R | PROT_W);
        // Start the stack with some distance from the end of the memory.
        // The CRT expects a certain area for the data set up by the kernel
        // (args, env, auxv).
        sp - 64
    };
    state.write_reg(SP, sp);

    'outer: loop {
        match state.interpret() {
            ExitReason::Ecall => {
                if matches!(handle_syscall(&mut state), None) {
                    break 'outer;
                }
            }
            r @ _ => todo!("unhandled exit reason {:?}", r),
        }
    }
}
