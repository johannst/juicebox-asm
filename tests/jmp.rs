use juicebox_asm::insn::Jmp;
use juicebox_asm::{Asm, Label};

#[test]
#[should_panic]
fn unbound_label() {
    let _l = Label::new();
}

#[test]
#[should_panic]
fn unbound_label2() {
    let mut lbl = Label::new();
    let mut asm = Asm::new();
    asm.jmp(&mut lbl);
}

#[test]
fn jmp_label() {
    {
        // Bind first.
        let mut lbl = Label::new();
        let mut asm = Asm::new();
        asm.bind(&mut lbl);
        asm.jmp(&mut lbl);
        // 0xfffffffb -> -5
        assert_eq!(asm.into_code(), [0xe9, 0xfb, 0xff, 0xff, 0xff]);
    }
    {
        // Bind later.
        let mut lbl = Label::new();
        let mut asm = Asm::new();
        asm.jmp(&mut lbl);
        asm.bind(&mut lbl);
        assert_eq!(asm.into_code(), [0xe9, 0x00, 0x00, 0x00, 0x00]);
    }
}

#[test]
fn jmp_label2() {
    {
        let mut lbl = Label::new();
        let mut asm = Asm::new();
        asm.jmp(&mut lbl);
        asm.nop();
        asm.nop();
        asm.bind(&mut lbl);
        assert_eq!(asm.into_code(), [0xe9, 0x02, 0x00, 0x00, 0x00, 0x90, 0x90]);
    }
    {
        let mut lbl = Label::new();
        let mut asm = Asm::new();
        asm.jmp(&mut lbl);
        for _ in 0..0x1ff {
            asm.nop();
        }
        asm.bind(&mut lbl);
        assert_eq!(asm.into_code()[..5], [0xe9, 0xff, 0x01, 0x00, 0x00]);
    }
}
