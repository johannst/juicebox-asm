# juicebox-asm

An `x64` jit assembler for learning purpose with the following two main goals:
- Learn about x64 instruction encoding.
- Learn how to use the rust type system to disallow invalid operands.

## Example

```rust
use juicebox_asm::prelude::{Reg32::*, *};
use juicebox_asm::rt::Runtime;

fn main() {
    let mut asm = Asm::new();

    // Reference implementation
    //   int ret = 0;
    //   int n   = 0;
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

## License
This project is licensed under the [MIT](LICENSE) license.
