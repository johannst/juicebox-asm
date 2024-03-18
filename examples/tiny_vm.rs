//! TinyVm example.
//!
//! This example introduces a simple 16 bit virtual machine the [`TinyVm`]. The VM consists of
//! three registers defined in [`TinyReg`], a separate _data_ and _instruction_ memory and a small
//! set of instructions [`TinyInsn`], sufficient to implement a guest program to compute the
//! Fibonacci sequence.
//!
//! The `TinyVm` implements a simple _just-in-time (JIT)_ compiler to demonstrate the
//! [`juicebox_asm`] crate. Additionally, it implements a reference _interpreter_.
//!
//! ```
//! fn main() {
//!   let mut prog = Vec::new();
//!   prog.push(TinyInsn::LoadImm(TinyReg::A, 100));
//!   prog.push(TinyInsn::Add(TinyReg::B, TinyReg::A));
//!   prog.push(TinyInsn::Addi(TinyReg::C, 100));
//!   prog.push(TinyInsn::Halt);
//!
//!   let mut vm = TinyVm::new(prog);
//!   vm.interp();
//!
//!   assert_eq!(100, vm.read_reg(TinyReg::A));
//!   assert_eq!(100, vm.read_reg(TinyReg::B));
//!   assert_eq!(100, vm.read_reg(TinyReg::C));
//!   assert_eq!(4, vm.icnt);
//!   assert_eq!(4, vm.pc);
//!
//!   vm.pc = 0;
//!   vm.jit();
//!
//!   assert_eq!(100, vm.read_reg(TinyReg::A));
//!   assert_eq!(200, vm.read_reg(TinyReg::B));
//!   assert_eq!(200, vm.read_reg(TinyReg::C));
//!   assert_eq!(8, vm.icnt);
//!   assert_eq!(4, vm.pc);
//! }
//! ```

use juicebox_asm::insn::*;
use juicebox_asm::Runtime;
use juicebox_asm::{Asm, Imm16, Imm64, MemOp, Reg16, Reg64};

/// A guest physical address.
pub struct PhysAddr(pub u16);

impl Into<usize> for PhysAddr {
    fn into(self) -> usize {
        self.0 as usize
    }
}

/// The registers for the [`TinyVm`].
#[derive(Debug, PartialEq, Clone, Copy)]
pub enum TinyReg {
    A,
    B,
    C,
}

impl TinyReg {
    #[inline]
    fn idx(&self) -> usize {
        *self as usize
    }
}

/// The instructions for the [`TinyVm`].
#[derive(Debug, PartialEq, Clone, Copy)]
pub enum TinyInsn {
    /// Halt the VM.
    Halt,
    /// Load the immediate value into the register `reg = imm`.
    LoadImm(TinyReg, u16),
    /// Load a value from the memory (absolute addressing) into the register `reg = mem[imm]`.
    Load(TinyReg, u16),
    /// Store a value from the register into the memory (absolute addressing) `mem[imm] = reg`.
    Store(TinyReg, u16),
    /// Add the register to the register `reg1 += reg2`.
    Add(TinyReg, TinyReg),
    /// Add the immediate to the register `reg += imm`.
    Addi(TinyReg, i16),
    /// Jump unconditional (absolute addressing) `pc = disp`.
    Branch(usize),
    /// Jump if the register is zero (absolute addressing) `pc = (reg == 0) ? disp : pc++`.
    BranchZero(TinyReg, usize),
}

/// Value returned from a [`JitFn`].
#[repr(C)]
struct JitRet(u64, u64);

/// Function signature defining the simple JIT ABI used in this example.
/// A `JitFn` represents the entry point to a jit compiled _basic block_ of the guest software.
///
/// ```text
/// JIT entry:
///     arg0: pointer to guest registers
///     arg1: pointer to guest data memory
///
/// JIT exit:
///      JitRet(0, N): Halt instruction, executed N instructions.
///      JitRet(N, R): N!=0
///                    End of basic block, executed N instructions,
///                    must re-enter at `pc = R`.
/// ```
type JitFn = extern "C" fn(*mut u16, *mut u8) -> JitRet;

/// The `TinyVm` virtual machine state.
pub struct TinyVm {
    /// Data memory, covering full 16 bit guest address space.
    ///
    /// For simplicity add additional trailing 1 byte to support an unaligned access to 0xffff
    /// without any special handling.
    dmem: [u8; 0x1_0000 + 1],
    /// Instruction memory.
    imem: Vec<TinyInsn>,
    /// VM registers.
    regs: [u16; 3],
    /// VM program counter.
    pc: usize,
    /// VM executed instruction counter (perf counter).
    icnt: usize,

    // -- JIT state.
    /// Mapping of guest PCs to jitted host code (`JitFn`). This mapping is filled when guest
    /// _basic blocks_ are jitted.
    jit_cache: Vec<Option<JitFn>>,
    /// JIT runtime maintaining the host pages containing the jitted guest code.
    rt: Runtime,
}

impl TinyVm {
    /// Create a new [`TinyVm`] and initialize the instruction memory from `code`.
    pub fn new(code: Vec<TinyInsn>) -> Self {
        let mut jit_cache = Vec::with_capacity(code.len());
        jit_cache.resize(code.len(), None);

        TinyVm {
            dmem: [0; 0x1_0000 + 1],
            imem: code,
            regs: [0; 3],
            pc: 0,
            icnt: 0,
            // -- JIT state.
            jit_cache,
            rt: Runtime::new(),
            // Confifigure the runtime to generates perf meta data.
            //rt: Runtime::with_profile(),
        }
    }

    /// Read guest register.
    #[inline]
    pub fn read_reg(&self, reg: TinyReg) -> u16 {
        self.regs[reg.idx()]
    }

    /// Write guest register.
    #[inline]
    pub fn write_reg(&mut self, reg: TinyReg, val: u16) {
        self.regs[reg.idx()] = val;
    }

    /// Read guest data memory.
    #[inline]
    pub fn read_mem(&self, paddr: PhysAddr) -> u16 {
        // dmem covers whole 16 bit address space + 1 byte for unaligned access at 0xffff.
        let bytes = self.dmem[paddr.into()..][..2].try_into().unwrap();
        u16::from_le_bytes(bytes)
    }

    /// Write guest data memory.
    #[inline]
    pub fn write_mem(&mut self, paddr: PhysAddr, val: u16) {
        let bytes = val.to_le_bytes();
        self.dmem[paddr.into()..][..2].copy_from_slice(&bytes);
    }

    /// Dump the VM state to stdout.
    pub fn dump(&self) {
        println!("-- TinyVm state --");
        println!("  ICNT: {}", self.icnt);
        println!("  PC  : {:02x}", self.pc - 1);
        println!(
            "  A:{:04x} B:{:04x} C:{:04x}",
            self.read_reg(TinyReg::A),
            self.read_reg(TinyReg::B),
            self.read_reg(TinyReg::C),
        );
    }

    /// Run in interpreter mode until the next [`TinyInsn::Halt`] instruction is hit.
    pub fn interp(&mut self) {
        'outer: loop {
            let insn = self.imem[self.pc];
            //println!("[0x{:02x}] {:?}", self.pc, insn);

            self.pc = self.pc.wrapping_add(1);
            self.icnt += 1;

            match insn {
                TinyInsn::Halt => {
                    break 'outer;
                }
                TinyInsn::LoadImm(a, imm) => {
                    self.write_reg(a, imm);
                }
                TinyInsn::Load(a, addr) => {
                    let val = self.read_mem(PhysAddr(addr));
                    self.write_reg(a, val);
                }
                TinyInsn::Store(a, addr) => {
                    let val = self.read_reg(a);
                    self.write_mem(PhysAddr(addr), val);
                }
                TinyInsn::Add(a, b) => {
                    let res = self.read_reg(a).wrapping_add(self.read_reg(b));
                    self.write_reg(a, res);
                }
                TinyInsn::Addi(a, imm) => {
                    let res = self.read_reg(a).wrapping_add(imm as u16);
                    self.write_reg(a, res);
                }
                TinyInsn::Branch(disp) => {
                    self.pc = disp;
                }
                TinyInsn::BranchZero(a, disp) => {
                    if self.read_reg(a) == 0 {
                        self.pc = disp;
                    }
                }
            }
        }
    }

    /// Run in JIT mode until the next [`TinyInsn::Halt`] instruction is hit. Translate guest
    /// _basic blocks_ on demand.
    pub fn jit(&mut self) {
        'outer: loop {
            let bb_fn = if let Some(bb_fn) = self.jit_cache[self.pc] {
                bb_fn
            } else {
                let bb_fn = self.translate_next_bb();
                self.jit_cache[self.pc] = Some(bb_fn);
                //println!("[0x{:02x}] translated bb at {:p}", self.pc, bb_fn);
                bb_fn
            };

            match bb_fn(self.regs.as_mut_ptr(), self.dmem.as_mut_ptr()) {
                // HALT instruction hit.
                JitRet(0, insn) => {
                    self.pc += insn as usize;
                    self.icnt += insn as usize;
                    break 'outer;
                }
                // End of basic block, re-enter.
                JitRet(insn, reenter_pc) => {
                    self.pc = reenter_pc as usize;
                    self.icnt += insn as usize;
                }
            }
        }
    }

    #[cfg(all(any(target_arch = "x86_64", target_os = "linux")))]
    /// Translate the bb at the current pc and return a JitFn pointer to it.
    fn translate_next_bb(&mut self) -> JitFn {
        let mut bb = Asm::new();
        let mut pc = self.pc;

        'outer: loop {
            let insn = self.imem[pc];

            pc = pc.wrapping_add(1);

            // JIT abi: JitFn -> JitRet
            //
            // According to SystemV abi:
            //   enter
            //     rdi => regs
            //     rsi => dmem
            //   exit
            //     rax => JitRet.0
            //     rdx => JitRet.1

            // Generate memory operand into regs for guest register.
            let reg_op = |r: TinyReg| {
                MemOp::IndirectDisp(Reg64::rdi, (r.idx() * 2).try_into().expect("only 3 regs"))
            };

            // Generate memory operand into dmem for guest phys address.
            let mem_op = |paddr: u16| MemOp::IndirectDisp(Reg64::rsi, paddr.into());

            // Compute instructions in translated basic block.
            let bb_icnt = || -> u64 { (pc - self.pc).try_into().unwrap() };

            let reenter_pc = |pc: usize| -> u64 { pc.try_into().unwrap() };

            match insn {
                TinyInsn::Halt => {
                    bb.mov(Reg64::rax, Imm64::from(0));
                    bb.mov(Reg64::rdx, Imm64::from(bb_icnt()));
                    bb.ret();
                    break 'outer;
                }
                TinyInsn::LoadImm(a, imm) => {
                    bb.mov(reg_op(a), Imm16::from(imm));
                }
                TinyInsn::Load(a, addr) => {
                    bb.mov(Reg16::ax, mem_op(addr));
                    bb.mov(reg_op(a), Reg16::ax);
                }
                TinyInsn::Store(a, addr) => {
                    bb.mov(Reg16::ax, reg_op(a));
                    bb.mov(mem_op(addr), Reg16::ax);
                }
                TinyInsn::Add(a, b) => {
                    bb.mov(Reg16::ax, reg_op(b));
                    bb.add(reg_op(a), Reg16::ax);
                }
                TinyInsn::Addi(a, imm) => {
                    bb.add(reg_op(a), Imm16::from(imm));
                }
                TinyInsn::Branch(disp) => {
                    bb.mov(Reg64::rax, Imm64::from(bb_icnt()));
                    bb.mov(Reg64::rdx, Imm64::from(reenter_pc(disp)));
                    bb.ret();
                    break 'outer;
                }
                TinyInsn::BranchZero(a, disp) => {
                    bb.cmp(reg_op(a), Imm16::from(0u16));
                    bb.mov(Reg64::rax, Imm64::from(bb_icnt()));
                    // Default fall-through PC (branch not taken).
                    bb.mov(Reg64::rdx, Imm64::from(reenter_pc(pc)));

                    // Conditionally update PC if condition is ZERO (branch taken).
                    bb.mov(Reg64::r11, Imm64::from(reenter_pc(disp)));
                    bb.cmovz(Reg64::rdx, Reg64::r11);

                    bb.ret();
                    break 'outer;
                }
            }
        }

        unsafe { self.rt.add_code::<JitFn>(bb.into_code()) }
    }
}

/// A minial fixup utility to implement jump labels when constructing guest programs.
pub struct Fixup {
    pc: usize,
}

impl Fixup {
    /// Create a new `Fixup` at the current pc.
    pub fn new(pc: usize) -> Self {
        Fixup { pc }
    }

    /// Bind the `Fixup` to the current location of `prog` and resolve the `Fixup`.
    pub fn bind(self, prog: &mut Vec<TinyInsn>) {
        let plen = prog.len();
        let insn = prog.get_mut(self.pc).expect(&format!(
            "Trying to apply Fixup, but Fixup is out of range pc={} prog.len={}",
            self.pc, plen
        ));

        match insn {
            TinyInsn::Branch(disp) | TinyInsn::BranchZero(_, disp) => {
                *disp = plen;
            }
            _ => {
                unimplemented!("Trying to fixup non-branch instruction '{:?}'", *insn);
            }
        }
    }
}

/// Generate a guest program to compute the fiibonacci sequence for `n`.
pub fn make_tinyvm_fib(start_n: u16) -> Vec<TinyInsn> {
    // Reference implementation:
    //
    // int fib(int n)
    //   int tmp = 0;
    //   int prv = 1;
    //   int sum = 0;
    // loop:
    //   if (n == 0) goto end;
    //   tmp = sum;
    //   sum += prv;
    //   prv = tmp;
    //   --n;
    //   goto loop;
    // end:
    //   return sum;

    // Variables live in memory, bin to fixed addresses.
    let tmp = 0u16;
    let prv = 2u16;
    let sum = 4u16;
    // Loop counter mapped to register.
    let n = TinyReg::C;

    let mut prog = Vec::with_capacity(32);

    // n = start_n
    prog.push(TinyInsn::LoadImm(n, start_n));

    // tmp = sum = 0
    prog.push(TinyInsn::LoadImm(TinyReg::A, 0));
    prog.push(TinyInsn::Store(TinyReg::A, tmp));
    prog.push(TinyInsn::Store(TinyReg::A, sum));

    // prv = 1
    prog.push(TinyInsn::LoadImm(TinyReg::A, 1));
    prog.push(TinyInsn::Store(TinyReg::A, prv));

    // Create loop_start label.
    let loop_start = prog.len();

    // Create fixup to capture PC that need to be patched later.
    let end_fixup = Fixup::new(prog.len());

    // if (n == 0) goto end
    prog.push(TinyInsn::BranchZero(n, 0xdead));

    // tmp = sum
    prog.push(TinyInsn::Load(TinyReg::A, sum));
    prog.push(TinyInsn::Store(TinyReg::A, tmp));

    // sum += prv
    prog.push(TinyInsn::Load(TinyReg::B, prv));
    prog.push(TinyInsn::Add(TinyReg::A, TinyReg::B));
    prog.push(TinyInsn::Store(TinyReg::A, sum));

    // prv = tmp
    prog.push(TinyInsn::Load(TinyReg::A, tmp));
    prog.push(TinyInsn::Store(TinyReg::A, prv));

    // --n
    prog.push(TinyInsn::Addi(n, -1));

    // goto loop_start
    prog.push(TinyInsn::Branch(loop_start));

    // Bind end fixup to current PC, to patch branch to jump to here.
    end_fixup.bind(&mut prog);

    // TinyReg::A = sum
    prog.push(TinyInsn::Load(TinyReg::A, sum));

    // Halt the VM.
    prog.push(TinyInsn::Halt);

    prog
}

/// Generate a test program for the jit.
pub fn make_tinyvm_jit_test() -> Vec<TinyInsn> {
    let mut prog = Vec::with_capacity(32);

    prog.push(TinyInsn::Branch(1));
    prog.push(TinyInsn::LoadImm(TinyReg::A, 0x0010));
    prog.push(TinyInsn::LoadImm(TinyReg::B, 0x0));

    let start = prog.len();
    let end = Fixup::new(prog.len());
    prog.push(TinyInsn::BranchZero(TinyReg::A, 0xdead));
    prog.push(TinyInsn::LoadImm(TinyReg::C, 0x1));
    prog.push(TinyInsn::Add(TinyReg::B, TinyReg::C));
    prog.push(TinyInsn::Addi(TinyReg::A, -1));
    prog.push(TinyInsn::Branch(start));
    end.bind(&mut prog);
    prog.push(TinyInsn::LoadImm(TinyReg::A, 0xabcd));
    prog.push(TinyInsn::Store(TinyReg::A, 0xffff));
    prog.push(TinyInsn::Load(TinyReg::C, 0xffff));
    prog.push(TinyInsn::Halt);
    prog.push(TinyInsn::Halt);
    prog.push(TinyInsn::Halt);
    prog.push(TinyInsn::Halt);

    prog
}

/// Generate a simple count down loop to crunch some instructions.
pub fn make_tinyvm_jit_perf() -> Vec<TinyInsn> {
    let mut prog = Vec::with_capacity(32);

    prog.push(TinyInsn::Halt);
    prog.push(TinyInsn::LoadImm(TinyReg::A, 0xffff));
    prog.push(TinyInsn::LoadImm(TinyReg::B, 1));
    prog.push(TinyInsn::LoadImm(TinyReg::C, 2));
    prog.push(TinyInsn::Addi(TinyReg::A, -1));
    prog.push(TinyInsn::BranchZero(TinyReg::A, 0));
    prog.push(TinyInsn::Branch(2));
    prog
}

fn main() {
    let use_jit = match std::env::args().nth(1) {
        Some(a) if a == "-h" || a == "--help" => {
            println!("Usage: tiny_vm [mode]");
            println!("");
            println!("Options:");
            println!("    mode    if mode is 'jit' then run in jit mode, else in interpreter mode");
            std::process::exit(0);
        }
        Some(a) if a == "jit" => true,
        _ => false,
    };

    let mut vm = TinyVm::new(make_tinyvm_fib(42));

    if use_jit {
        println!("Run in jit mode..");
        vm.jit();
    } else {
        println!("Run in interpreter mode..");
        vm.interp();
    }
    vm.dump();
}

#[cfg(test)]
mod test {
    use super::*;

    fn fib_rs(n: u64) -> u64 {
        if n < 2 {
            n
        } else {
            let mut fib_n_m1 = 0;
            let mut fib_n = 1;
            for _ in 1..n {
                let tmp = fib_n + fib_n_m1;
                fib_n_m1 = fib_n;
                fib_n = tmp;
            }
            fib_n
        }
    }

    #[test]
    fn test_fib_interp() {
        for n in 0..92 {
            let mut vm = TinyVm::new(make_tinyvm_fib(n));
            vm.interp();

            assert_eq!((fib_rs(n as u64) & 0xffff) as u16, vm.read_reg(TinyReg::A));
        }
    }

    #[test]
    fn test_fib_jit() {
        for n in 0..92 {
            let mut vm = TinyVm::new(make_tinyvm_fib(n));
            vm.jit();

            assert_eq!((fib_rs(n as u64) & 0xffff) as u16, vm.read_reg(TinyReg::A));
        }
    }

    #[test]
    fn test_fib_icnt() {
        let mut vm1 = TinyVm::new(make_tinyvm_fib(91));
        vm1.interp();
        let mut vm2 = TinyVm::new(make_tinyvm_fib(91));
        vm2.jit();

        assert_eq!(vm1.icnt, vm2.icnt);
        assert_eq!(vm1.pc, vm2.pc);
    }

    #[test]
    fn test_jit_load_imm() {
        let mut prog = Vec::new();
        prog.push(TinyInsn::LoadImm(TinyReg::A, 0x1111));
        prog.push(TinyInsn::LoadImm(TinyReg::B, 0x2222));
        prog.push(TinyInsn::LoadImm(TinyReg::C, 0x3333));
        prog.push(TinyInsn::Halt);

        let mut vm = TinyVm::new(prog);
        vm.jit();

        assert_eq!(0x1111, vm.read_reg(TinyReg::A));
        assert_eq!(0x2222, vm.read_reg(TinyReg::B));
        assert_eq!(0x3333, vm.read_reg(TinyReg::C));

        assert_eq!(4, vm.icnt);
        assert_eq!(4, vm.pc);
    }

    #[test]
    fn test_jit_add() {
        let mut prog = Vec::new();
        prog.push(TinyInsn::LoadImm(TinyReg::A, 0));
        prog.push(TinyInsn::Addi(TinyReg::A, 123));

        prog.push(TinyInsn::LoadImm(TinyReg::B, 100));
        prog.push(TinyInsn::LoadImm(TinyReg::C, 200));
        prog.push(TinyInsn::Add(TinyReg::B, TinyReg::C));
        prog.push(TinyInsn::Halt);

        let mut vm = TinyVm::new(prog);
        vm.jit();

        assert_eq!(123, vm.read_reg(TinyReg::A));
        assert_eq!(300, vm.read_reg(TinyReg::B));
        assert_eq!(200, vm.read_reg(TinyReg::C));

        assert_eq!(6, vm.icnt);
        assert_eq!(6, vm.pc);
    }

    #[test]
    fn test_jit_load_store() {
        let mut prog = Vec::new();
        prog.push(TinyInsn::Load(TinyReg::A, 0xffff));

        prog.push(TinyInsn::LoadImm(TinyReg::B, 0xf00d));
        prog.push(TinyInsn::Store(TinyReg::B, 0x8000));
        prog.push(TinyInsn::Halt);

        let mut vm = TinyVm::new(prog);
        vm.write_mem(PhysAddr(0xffff), 0xaabb);
        vm.jit();

        assert_eq!(0xaabb, vm.read_reg(TinyReg::A));
        assert_eq!(0xf00d, vm.read_mem(PhysAddr(0x8000)));

        assert_eq!(4, vm.icnt);
        assert_eq!(4, vm.pc);
    }

    #[test]
    fn test_jit_branch() {
        let mut prog = Vec::new();
        prog.push(TinyInsn::Branch(2));
        prog.push(TinyInsn::Halt);
        prog.push(TinyInsn::Branch(6));
        prog.push(TinyInsn::LoadImm(TinyReg::A, 1));
        prog.push(TinyInsn::LoadImm(TinyReg::B, 2));
        prog.push(TinyInsn::LoadImm(TinyReg::C, 3));
        prog.push(TinyInsn::Branch(1));

        let mut vm = TinyVm::new(prog);
        vm.jit();

        assert_eq!(0, vm.read_reg(TinyReg::A));
        assert_eq!(0, vm.read_reg(TinyReg::B));
        assert_eq!(0, vm.read_reg(TinyReg::C));

        assert_eq!(4, vm.icnt);
        assert_eq!(2, vm.pc);
    }

    #[test]
    fn test_jit_branch_zero() {
        let mut prog = Vec::new();
        prog.push(TinyInsn::LoadImm(TinyReg::A, 1));
        prog.push(TinyInsn::BranchZero(TinyReg::A, 5));
        prog.push(TinyInsn::LoadImm(TinyReg::A, 0));
        prog.push(TinyInsn::BranchZero(TinyReg::A, 5));
        prog.push(TinyInsn::LoadImm(TinyReg::B, 22));
        prog.push(TinyInsn::Halt);

        let mut vm = TinyVm::new(prog);
        vm.jit();

        assert_eq!(0, vm.read_reg(TinyReg::A));
        assert_eq!(0, vm.read_reg(TinyReg::B));
        assert_eq!(0, vm.read_reg(TinyReg::C));

        assert_eq!(5, vm.icnt);
        assert_eq!(6, vm.pc);
    }

    #[test]
    fn test_mixed() {
        let mut prog = Vec::new();
        prog.push(TinyInsn::LoadImm(TinyReg::A, 100));
        prog.push(TinyInsn::Add(TinyReg::B, TinyReg::A));
        prog.push(TinyInsn::Addi(TinyReg::C, 100));
        prog.push(TinyInsn::Halt);

        let mut vm = TinyVm::new(prog);
        vm.interp();

        assert_eq!(100, vm.read_reg(TinyReg::A));
        assert_eq!(100, vm.read_reg(TinyReg::B));
        assert_eq!(100, vm.read_reg(TinyReg::C));
        assert_eq!(4, vm.icnt);
        assert_eq!(4, vm.pc);

        vm.pc = 0;
        vm.jit();

        assert_eq!(100, vm.read_reg(TinyReg::A));
        assert_eq!(200, vm.read_reg(TinyReg::B));
        assert_eq!(200, vm.read_reg(TinyReg::C));
        assert_eq!(8, vm.icnt);
        assert_eq!(4, vm.pc);
    }
}
