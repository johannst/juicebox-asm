//! A simple `x64` jit assembler with a minimal runtime to execute emitted code for fun.
//!
//! The following is an fibonacci example implementation.
//! ```rust
//! use juicebox_asm::{Asm, Reg64, Imm64, Label};
//! use juicebox_asm::insn::*;
//! use juicebox_asm::Runtime;
//!
//! const fn fib_rs(n: u64) -> u64 {
//!     match n {
//!         0 => 0,
//!         1 => 1,
//!         _ => fib_rs(n - 2) + fib_rs(n - 1),
//!     }
//! }
//!
//! fn main() {
//!     let mut asm = Asm::new();
//!
//!     let mut lp = Label::new();
//!     let mut end = Label::new();
//!
//!     // Reference implementation:
//!     //
//!     // int fib(int n) {
//!     //   int tmp = 0;
//!     //   int prv = 1;
//!     //   int sum = 0;
//!     // loop:
//!     //   if (n == 0) goto end;
//!     //   tmp = sum;
//!     //   sum += prv;
//!     //   prv = tmp;
//!     //   --n;
//!     //   goto loop;
//!     // end:
//!     //   return sum;
//!     // }
//!
//!     // SystemV abi:
//!     //   rdi -> first argument
//!     //   rax -> return value
//!     let n = Reg64::rdi;
//!     let sum = Reg64::rax;
//!
//!     let tmp = Reg64::rcx;
//!     let prv = Reg64::rbx;
//!
//!     asm.mov(tmp, Imm64::from(0));
//!     asm.mov(prv, Imm64::from(1));
//!     asm.mov(sum, Imm64::from(0));
//!
//!     asm.bind(&mut lp);
//!     asm.test(n, n);
//!     asm.jz(&mut end);
//!     asm.mov(tmp, sum);
//!     asm.add(sum, prv);
//!     asm.mov(prv, tmp);
//!     asm.dec(n);
//!     asm.jmp(&mut lp);
//!     asm.bind(&mut end);
//!     asm.ret();
//!
//!     // Move code into executable page and get function pointer to it.
//!     let mut rt = Runtime::new();
//!     let fib = unsafe { rt.add_code::<extern "C" fn(u64) -> u64>(&asm.into_code()) };
//!
//!     for n in 0..15 {
//!         let fib_jit = fib(n);
//!         println!("fib({}) = {}", n, fib_jit);
//!         assert_eq!(fib_jit, fib_rs(n));
//!     }
//! }
//! ```

mod asm;
mod imm;
mod label;
mod reg;
mod rt;

pub mod insn;

pub use asm::Asm;
pub use imm::{Imm16, Imm32, Imm64, Imm8};
pub use label::Label;
pub use reg::{Reg16, Reg32, Reg64, Reg8};
pub use rt::Runtime;

/// Type representing a memory operand.
#[derive(Clone, Copy)]
pub enum MemOp {
    /// An indirect memory operand, eg `mov [rax], rcx`.
    Indirect(Reg64),

    /// An indirect memory operand with additional displacement, eg `mov [rax + 0x10], rcx`.
    IndirectDisp(Reg64, i32),

    /// An indirect memory operand in the form base + index, eg `mov [rax + rcx], rdx`.
    IndirectBaseIndex(Reg64, Reg64),
}

impl MemOp {
    /// Get the base address register of the memory operand.
    const fn base(&self) -> Reg64 {
        match self {
            MemOp::Indirect(base) => *base,
            MemOp::IndirectDisp(base, ..) => *base,
            MemOp::IndirectBaseIndex(base, ..) => *base,
        }
    }

    /// Get the index register of the memory operand.
    fn index(&self) -> Reg64 {
        // Return zero index register for memory operands w/o index register.
        let zero_index = Reg64::rax;
        use reg::Reg;
        assert_eq!(zero_index.idx(), 0);

        match self {
            MemOp::Indirect(..) => zero_index,
            MemOp::IndirectDisp(..) => zero_index,
            MemOp::IndirectBaseIndex(.., index) => *index,
        }
    }
}

/// Trait to give size hints for memory operands.
trait MemOpSized {
    fn mem_op(&self) -> MemOp;
}

macro_rules! impl_memop_sized {
    ($(#[$doc:meta] $name:ident)+) => {
        $(
        #[$doc]
        pub struct $name(MemOp);

        impl $name {
            /// Create a memory with size hint from a raw memory operand.
            pub fn from(op: MemOp) -> Self {
                Self(op)
            }
        }

        impl MemOpSized for $name {
            fn mem_op(&self) -> MemOp {
                self.0
            }
        }
        )+
    };
}

impl_memop_sized!(
    /// A memory operand with a word (8 bit) size hint.
    MemOp8
    /// A memory operand with a word (16 bit) size hint.
    MemOp16
    /// A memory operand with a dword (32 bit) size hint.
    MemOp32
    /// A memory operand with a qword (64 bit) size hint.
    MemOp64
);
