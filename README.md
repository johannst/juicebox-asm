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
use juicebox_asm::prelude::{Reg32::*, *};
use juicebox_asm::Runtime;

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

    let rt = Runtime::new(&asm.into_code());
    let func = unsafe { rt.as_fn::<extern "C" fn() -> u32>() };
    assert_eq!(func(), (0..=42).into_iter().sum());
}
```

The [`examples/`](examples/) folder provides additional examples:

- [`fib.rs`](examples/fib.rs) jit compile a function to compute the `fibonacci` sequence.
- [`add.rs`](examples/add.rs) jit compile a function calling another function compiled into the example.
- [`tiny_vm.rs`](examples/tiny_vm.rs.rs) define a minimal `virtual machine (VM)` which demonstrates a simple jit compiler for translating VM guest software.

## License
This project is licensed under the [MIT](LICENSE) license.
