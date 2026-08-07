// SPDX-License-Identifier: MIT
//
// Copyright (c) 2023, Johannes Stoelp <dev@memzero.de>

//! Trait definitions of various instructions.

mod add;
mod and;
mod call;
mod cmovnz;
mod cmovz;
mod cmp;
mod dec;
mod inc;
mod int3;
mod jae;
mod jb;
mod jge;
mod jl;
mod jmp;
mod jnz;
mod jz;
mod lea;
mod mov;
mod movsx;
mod movzx;
mod nop;
mod or;
mod pop;
mod push;
mod ret;
mod sar;
mod setb;
mod setl;
mod shl;
mod shr;
mod sub;
mod test;
mod xor;

/// Trait for [`add`](https://www.felixcloutier.com/x86/add) instruction kinds.
pub trait Add<T, U> {
    /// Emit an add instruction.
    fn add(&mut self, op1: T, op2: U);
}

/// Trait for [`and`](https://www.felixcloutier.com/x86/and) instruction kinds.
pub trait And<T, U> {
    /// Emit an and instruction.
    fn and(&mut self, op1: T, op2: U);
}

/// Trait for [`call`](https://www.felixcloutier.com/x86/call) instruction kinds.
pub trait Call<T> {
    /// Emit a call instruction.
    fn call(&mut self, op1: T);
}

/// Trait for [`cmovnz`](https://www.felixcloutier.com/x86/cmovcc) instruction kinds.
pub trait Cmovnz<T, U> {
    /// Emit a (conditional) move if not zero instruction.
    ///
    /// Move is only commited if (ZF=0).
    fn cmovnz(&mut self, op1: T, op2: U);
}

/// Trait for [`cmovz`](https://www.felixcloutier.com/x86/cmovcc) instruction kinds.
pub trait Cmovz<T, U> {
    /// Emit a (conditional) move if zero instruction.
    ///
    /// Move is only commited if (ZF=1).
    fn cmovz(&mut self, op1: T, op2: U);
}

/// Trait for [`cmp`](https://www.felixcloutier.com/x86/cmp) instruction kinds.
pub trait Cmp<T, U> {
    /// Emit a compare instruction.
    ///
    /// Computes `op2 - op1` and sets the status flags in the same way as the `sub` instruction,
    /// the result is discarded.
    fn cmp(&mut self, op1: T, op2: U);
}

/// Trait for [`dec`](https://www.felixcloutier.com/x86/dec) instruction kinds.
pub trait Dec<T> {
    /// Emit a decrement instruction.
    fn dec(&mut self, op1: T);
}

/// Trait for [`inc`](https://www.felixcloutier.com/x86/inc) instruction kinds.
pub trait Inc<T> {
    /// Emit a increment instruction.
    fn inc(&mut self, op1: T);
}

/// Trait for [`jae`](https://www.felixcloutier.com/x86/jcc) instruction kinds.
pub trait Jae<T> {
    /// Emit a conditional jump if above or equal (`CF=0`).
    /// This handles the flags from an unsigned arithmetic operation.
    fn jae(&mut self, op1: T);
}

/// Trait for [`jb`](https://www.felixcloutier.com/x86/jcc) instruction kinds.
pub trait Jb<T> {
    /// Emit a conditional jump if below (`CF=1`).
    /// This handles the flags from an unsigned arithmetic operation.
    fn jb(&mut self, op1: T);
}

/// Trait for [`jge`](https://www.felixcloutier.com/x86/jcc) instruction kinds.
pub trait Jge<T> {
    /// Emit a conditional jump if greater or equal (`SF=OF`).
    /// This handles the flags from a signed arithmetic operation.
    fn jge(&mut self, op1: T);
}

/// Trait for [`jl`](https://www.felixcloutier.com/x86/jcc) instruction kinds.
pub trait Jl<T> {
    /// Emit a conditional jump if less (`SF!=OF`).
    /// This handles the flags from a signed arithmetic operation.
    fn jl(&mut self, op1: T);
}

/// Trait for [`jmp`](https://www.felixcloutier.com/x86/jmp) instruction kinds.
pub trait Jmp<T> {
    /// Emit an unconditional jump instruction.
    fn jmp(&mut self, op1: T);
}

/// Trait for [`jnz`](https://www.felixcloutier.com/x86/jcc) instruction kinds.
pub trait Jnz<T> {
    /// Emit a conditional jump if not zero instruction (`ZF = 0`).
    fn jnz(&mut self, op1: T);
}

/// Trait for [`jz`](https://www.felixcloutier.com/x86/jcc) instruction kinds.
pub trait Jz<T> {
    /// Emit a conditional jump if zero instruction (`ZF = 1`).
    fn jz(&mut self, op1: T);
}

/// Trait for [`lea`](https://www.felixcloutier.com/x86/lea) instruction kinds.
pub trait Lea<T, U> {
    /// Emit a load effective address instruction.
    fn lea(&mut self, op1: T, op2: U);
}

/// Trait for [`mov`](https://www.felixcloutier.com/x86/mov) instruction kinds.
pub trait Mov<T, U> {
    /// Emit an move instruction.
    fn mov(&mut self, op1: T, op2: U);
}

/// Trait for [`movsx`](https://www.felixcloutier.com/x86/movsx:movsxd) instruction kinds.
pub trait Movsx<T, U> {
    /// Emit a sign-extend move instruction.
    fn movsx(&mut self, op1: T, op2: U);
}

/// Trait for [`movzx`](https://www.felixcloutier.com/x86/movzx) instruction kinds.
pub trait Movzx<T, U> {
    /// Emit a zero-extend move instruction.
    fn movzx(&mut self, op1: T, op2: U);
}

/// Trait for [`or`](https://www.felixcloutier.com/x86/or) instruction kinds.
pub trait Or<T, U> {
    /// Emit an or instruction.
    fn or(&mut self, op1: T, op2: U);
}

/// Trait for [`pop`](https://www.felixcloutier.com/x86/pop) instruction kinds.
pub trait Pop<T> {
    /// Emit a pop instruction.
    fn pop(&mut self, op1: T);
}

/// Trait for [`push`](https://www.felixcloutier.com/x86/push) instruction kinds.
pub trait Push<T> {
    /// Emit a push instruction.
    fn push(&mut self, op1: T);
}

/// Trait for [`sar`](https://www.felixcloutier.com/x86/sal:sar:shl:shr) instruction kinds.
pub trait Sar<T, U> {
    /// Emit an arithmethic shift-right instruction.
    fn sar(&mut self, op1: T, op2: U);
}

/// Trait for [`setb`](https://www.felixcloutier.com/x86/setcc) instruction kinds.
pub trait Setb<T> {
    /// Emit a set if below instruction.
    /// This handles the flags from an unsigned arithmetic operation.
    fn setb(&mut self, op1: T);
}

/// Trait for [`setl`](https://www.felixcloutier.com/x86/setcc) instruction kinds.
pub trait Setl<T> {
    /// Emit a set if less instruction.
    /// This handles the flags from an signed arithmetic operation.
    fn setl(&mut self, op1: T);
}

/// Trait for [`shl`](https://www.felixcloutier.com/x86/sal:sar:shl:shr) instruction kinds.
pub trait Shl<T, U> {
    /// Emit a logical shift-left instruction.
    fn shl(&mut self, op1: T, op2: U);
}

/// Trait for [`shr`](https://www.felixcloutier.com/x86/sal:sar:shl:shr) instruction kinds.
pub trait Shr<T, U> {
    /// Emit a logical shift-right instruction.
    fn shr(&mut self, op1: T, op2: U);
}

/// Trait for [`sub`](https://www.felixcloutier.com/x86/sub) instruction kinds.
pub trait Sub<T, U> {
    /// Emit an sub instruction.
    fn sub(&mut self, op1: T, op2: U);
}

/// Trait for [`test`](https://www.felixcloutier.com/x86/test) instruction kinds.
pub trait Test<T, U> {
    /// Emit a logical compare instruction.
    ///
    /// Computes the bit-wise logical AND of first operand and the second operand and sets the
    /// `SF`, `ZF`, and `PF` status flags, the result is discarded.
    fn test(&mut self, op1: T, op2: U);
}

/// Trait for [`xor`](https://www.felixcloutier.com/x86/xor) instruction kinds.
pub trait Xor<T, U> {
    /// Emit a xor instruction.
    fn xor(&mut self, op1: T, op2: U);
}
