use juicebox_asm::prelude::*;
use juicebox_asm::Runtime;
use Reg64::*;

extern "C" fn add(a: u32, b: u32) -> u32 {
    a + b
}

fn main() {
    let mut asm = Asm::new();

    // SystemV abi:
    //   rdi -> first argument
    //   rsi -> second argument
    //   rax -> return value
    //
    asm.mov(rsi, Imm64::from(42));
    asm.mov(rax, Imm64::from(add as u64));
    asm.call(rax);
    asm.ret();

    let code = asm.into_code();
    std::fs::write("jit.asm", &code).unwrap();

    let rt = Runtime::new(&code);
    let add42 = unsafe { rt.as_fn::<extern "C" fn(u32) -> u32>() };

    let res = add42(5);
    assert_eq!(res, 47);
}
