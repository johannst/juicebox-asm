//! Fibonacci example.
//!
//! Jit compile a function at runtime (generate native host code) to compute the fibonacci sequence
//! to demonstrate the [`juicebox_asm`] crate.

use juicebox_asm::insn::*;
use juicebox_asm::Runtime;
use juicebox_asm::{Asm, Imm64, Label, Reg64};

const fn fib_rs(n: u64) -> u64 {
    match n {
        0 => 0,
        1 => 1,
        _ => fib_rs(n - 2) + fib_rs(n - 1),
    }
}

fn main() {
    let mut asm = Asm::new();

    let mut lp = Label::new();
    let mut end = Label::new();

    // Reference implementation:
    //
    // int fib(int n) {
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
    // }

    // SystemV abi:
    //   rdi -> first argument
    //   rax -> return value
    let n = Reg64::rdi;
    let sum = Reg64::rax;

    let tmp = Reg64::rcx;
    let prv = Reg64::rdx;

    asm.mov(tmp, Imm64::from(0));
    asm.mov(prv, Imm64::from(1));
    asm.mov(sum, Imm64::from(0));

    asm.bind(&mut lp);
    asm.test(n, n);
    asm.jz(&mut end);
    asm.mov(tmp, sum);
    asm.add(sum, prv);
    asm.mov(prv, tmp);
    asm.dec(n);
    asm.jmp(&mut lp);
    asm.bind(&mut end);
    asm.ret();

    // Move code into executable page and get function pointer to it.
    let mut rt = Runtime::new();
    let fib = unsafe { rt.add_code::<extern "C" fn(u64) -> u64>(asm.into_code()) };

    // Disassemble JIT code and write to stdout.
    rt.disasm();

    for n in 0..15 {
        let fib_jit = fib(n);
        println!("fib({}) = {}", n, fib_jit);
        assert_eq!(fib_jit, fib_rs(n));
    }
}
