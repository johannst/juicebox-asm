# juicebox-asm

[![Rust][wf-badge]][wf-output] [![Rustdoc][doc-badge]][doc-html]

[wf-output]: https://github.com/johannst/juicebox-asm/actions/workflows/tests.yml
[wf-badge]: https://github.com/johannst/juicebox-asm/actions/workflows/tests.yml/badge.svg
[doc-html]: https://johannst.github.io/juicebox-asm
[doc-badge]: https://img.shields.io/badge/juicebox__asm-rustdoc-blue.svg?style=flat&logo=rust

An `x64` jit assembler for learning purpose with the following two main goals:

- Learn about x64 instruction encoding.
- Learn how to use the rust type system to disallow invalid operands.

## Example

```rust
use juicebox_asm::insn::*;
use juicebox_asm::Runtime;
use juicebox_asm::{Asm, Imm32, Label, Reg32::*};

fn main() {
    let mut asm = Asm::new();

    // Reference implementation
    //   int ret = 0;
    //   int n   = 42;
    //
    // loop:
    //   ret += n;
    //   --n;
    //   if (n != 0) goto loop;
    //
    //   return;

    let mut lp = Label::new();

    asm.mov(eax, Imm32::from(0));
    asm.mov(ecx, Imm32::from(42));

    asm.bind(&mut lp);
    asm.add(eax, ecx);
    asm.dec(ecx);
    asm.test(ecx, ecx);
    asm.jnz(&mut lp);

    asm.ret();

    let mut rt = Runtime::new();
    let func = unsafe { rt.add_code::<extern "C" fn() -> u32>(&asm.into_code()) };
    assert_eq!(func(), (0..=42).into_iter().sum());
}
```

The [`examples/`](examples/) folder provides additional examples:

- [`fib.rs`](examples/fib.rs) jit compiles a function to compute the
  `fibonacci` sequence.
- [`add.rs`](examples/add.rs) jit compiles a function calling another function
  compiled into the example binary.
- [`tiny_vm.rs`](examples/tiny_vm.rs) defines a minimal `virtual machine (VM)`
  with registers, instructions, data & instruction memory. The VM demonstrates
  a simple *jit compiler* which has a *jit cache* and translates each *basic
  block* on first execution when running a VM guest image. For reference an
  interepter is also implemented.
- [`bf_vm.rs`](examples/bf_vm.rs) implements a
  [brainfuck][https://en.wikipedia.org/wiki/Brainfuck] jit compiler
  and interpreter.

## git hook for local development

The [`ci/`](ci) checks can be run automatically during local development by
installing the following `pre-commit` git hook.
```sh
echo 'make -C ci' > .git/hooks/pre-commit
chmod +x .git/hooks/pre-commit
```

## License
This project is licensed under the [MIT](LICENSE) license.
