//! Trait definitions of various instructions.

mod add;
mod call;
mod cmovnz;
mod cmovz;
mod cmp;
mod dec;
mod inc;
mod jmp;
mod jnz;
mod jz;
mod mov;
mod nop;
mod pop;
mod push;
mod ret;
mod test;
mod xor;

/// Trait for [`add`](https://www.felixcloutier.com/x86/add) instruction kinds.
pub trait Add<T, U> {
    /// Emit an add instruction.
    fn add(&mut self, op1: T, op2: U);
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

/// Trait for [`mov`](https://www.felixcloutier.com/x86/mov) instruction kinds.
pub trait Mov<T, U> {
    /// Emit an move instruction.
    fn mov(&mut self, op1: T, op2: U);
}

/// Trait for [`push`](https://www.felixcloutier.com/x86/push) instruction kinds.
pub trait Push<T> {
    /// Emit a push instruction.
    fn push(&mut self, op1: T);
}

/// Trait for [`pop`](https://www.felixcloutier.com/x86/pop) instruction kinds.
pub trait Pop<T> {
    /// Emit a pop instruction.
    fn pop(&mut self, op1: T);
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
