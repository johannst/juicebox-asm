use juicebox_asm::Asm;
use juicebox_asm::MemOp;
use juicebox_asm::{Imm16, Imm32, Imm64, Imm8};
use juicebox_asm::{Reg16::*, Reg32::*, Reg64::*, Reg8::*};

macro_rules! mov {
    ($op1:expr, $op2:expr) => {{
        let mut asm = Asm::new();
        asm.mov($op1, $op2);
        asm.into_code()
    }};
}

#[rustfmt::skip]
#[test]
fn mov_rr() {
    // 64bit.
    assert_eq!(mov!(rcx, rdx), [0x48, 0x89, 0xd1]);
    assert_eq!(mov!(r11, rdx), [0x49, 0x89, 0xd3]);
    assert_eq!(mov!(rdi, r12), [0x4c, 0x89, 0xe7]);
    assert_eq!(mov!(r15, r12), [0x4d, 0x89, 0xe7]);

    // 32bit.
    assert_eq!(mov!(ecx,  edx),  [0x89, 0xd1]);
    assert_eq!(mov!(r11d, edx),  [0x41, 0x89, 0xd3]);
    assert_eq!(mov!(edi,  r12d), [0x44, 0x89, 0xe7]);
    assert_eq!(mov!(r15d, r12d), [0x45, 0x89, 0xe7]);

    // 16bit.
    assert_eq!(mov!(cx,   dx),   [0x66, 0x89, 0xd1]);
    assert_eq!(mov!(r11w, dx),   [0x66, 0x41, 0x89, 0xd3]);
    assert_eq!(mov!(di,   r12w), [0x66, 0x44, 0x89, 0xe7]);
    assert_eq!(mov!(r15w, r12w), [0x66, 0x45, 0x89, 0xe7]);

    // 8bit.
    assert_eq!(mov!(cl,   dl),   [0x88, 0xd1]);
    assert_eq!(mov!(ch,   dh),   [0x88, 0xf5]);
    assert_eq!(mov!(dil,  sil),  [0x40, 0x88, 0xf7]);
    assert_eq!(mov!(r11l, dl),   [0x41, 0x88, 0xd3]);
    assert_eq!(mov!(dil,  r12l), [0x44, 0x88, 0xe7]);
    assert_eq!(mov!(r15l, r12l), [0x45, 0x88, 0xe7]);
}

#[rustfmt::skip]
#[test]
fn mov_ri() {
    // 64bit.
    assert_eq!(mov!(rdi, Imm64::from(0xaabb)), [0x48, 0xbf, 0xbb, 0xaa, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00]);
    assert_eq!(mov!(r12, Imm64::from(0xaabb)), [0x49, 0xbc, 0xbb, 0xaa, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00]);

    // 32bit.
    assert_eq!(mov!(edi,  Imm32::from(0xaabb)), [0xbf, 0xbb, 0xaa, 0x00, 0x00]);
    assert_eq!(mov!(r12d, Imm32::from(0xaabb)), [0x41, 0xbc, 0xbb, 0xaa, 0x00, 0x00]);

    // 16bit.
    assert_eq!(mov!(di,   Imm16::from(0xaabbu16)), [0x66, 0xbf, 0xbb, 0xaa]);
    assert_eq!(mov!(r12w, Imm16::from(0xaabbu16)), [0x66, 0x41, 0xbc, 0xbb, 0xaa]);

    // 8bit.
    assert_eq!(mov!(dil,  Imm8::from(0xaau8)), [0x40, 0xb7, 0xaa]);
    assert_eq!(mov!(r12l, Imm8::from(0xaau8)), [0x41, 0xb4, 0xaa]);
}

#[rustfmt::skip]
#[test]
fn mov_rm() {
    // 64bit.
    assert_eq!(mov!(rcx, MemOp::Indirect(rdx)), [0x48, 0x8b, 0x0a]);
    assert_eq!(mov!(r11, MemOp::Indirect(rsi)), [0x4c, 0x8b, 0x1e]);
    assert_eq!(mov!(rdi, MemOp::Indirect(r14)), [0x49, 0x8b, 0x3e]);
    assert_eq!(mov!(r15, MemOp::Indirect(r14)), [0x4d, 0x8b, 0x3e]);

    // 32bit.
    assert_eq!(mov!(ecx,  MemOp::Indirect(rdx)), [0x8b, 0x0a]);
    assert_eq!(mov!(r11d, MemOp::Indirect(rsi)), [0x44, 0x8b, 0x1e]);
    assert_eq!(mov!(edi,  MemOp::Indirect(r14)), [0x41, 0x8b, 0x3e]);
    assert_eq!(mov!(r15d, MemOp::Indirect(r14)), [0x45, 0x8b, 0x3e]);

    // 16bit.
    assert_eq!(mov!(cx,   MemOp::Indirect(rdx)), [0x66, 0x8b, 0x0a]);
    assert_eq!(mov!(r11w, MemOp::Indirect(rsi)), [0x66, 0x44, 0x8b, 0x1e]);
    assert_eq!(mov!(di,   MemOp::Indirect(r14)), [0x66, 0x41, 0x8b, 0x3e]);
    assert_eq!(mov!(r15w, MemOp::Indirect(r14)), [0x66, 0x45, 0x8b, 0x3e]);

    // 8bit.
    assert_eq!(mov!(cl,   MemOp::Indirect(rdx)), [0x8a, 0x0a]);
    assert_eq!(mov!(r11l, MemOp::Indirect(rsi)), [0x44, 0x8a, 0x1e]);
    assert_eq!(mov!(dil,  MemOp::Indirect(r14)), [0x41, 0x8a, 0x3e]);
    assert_eq!(mov!(r15l, MemOp::Indirect(r14)), [0x45, 0x8a, 0x3e]);
}

#[rustfmt::skip]
#[test]
fn mov_mr() {
    // 64bit.
    assert_eq!(mov!(MemOp::Indirect(rdx), rcx), [0x48, 0x89, 0x0a]);
    assert_eq!(mov!(MemOp::Indirect(rsi), r11), [0x4c, 0x89, 0x1e]);
    assert_eq!(mov!(MemOp::Indirect(r14), rdi), [0x49, 0x89, 0x3e]);
    assert_eq!(mov!(MemOp::Indirect(r14), r15), [0x4d, 0x89, 0x3e]);

    // 32bit.
    assert_eq!(mov!(MemOp::Indirect(rdx), ecx),  [0x89, 0x0a]);
    assert_eq!(mov!(MemOp::Indirect(rsi), r11d), [0x44, 0x89, 0x1e]);
    assert_eq!(mov!(MemOp::Indirect(r14), edi),  [0x41, 0x89, 0x3e]);
    assert_eq!(mov!(MemOp::Indirect(r14), r15d), [0x45, 0x89, 0x3e]);

    // 16bit.
    assert_eq!(mov!(MemOp::Indirect(rdx), cx),   [0x66, 0x89, 0x0a]);
    assert_eq!(mov!(MemOp::Indirect(rsi), r11w), [0x66, 0x44, 0x89, 0x1e]);
    assert_eq!(mov!(MemOp::Indirect(r14), di),   [0x66, 0x41, 0x89, 0x3e]);
    assert_eq!(mov!(MemOp::Indirect(r14), r15w), [0x66, 0x45, 0x89, 0x3e]);

    // 8bit.
    assert_eq!(mov!(MemOp::Indirect(rdx), cl),   [0x88, 0x0a]);
    assert_eq!(mov!(MemOp::Indirect(rsi), r11l), [0x44, 0x88, 0x1e]);
    assert_eq!(mov!(MemOp::Indirect(r14), dil),  [0x41, 0x88, 0x3e]);
    assert_eq!(mov!(MemOp::Indirect(r14), r15l), [0x45, 0x88, 0x3e]);
}
