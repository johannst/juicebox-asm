//! Brainfuck VM.
//!
//! This example implements a simple
//! [brainfuck](https://en.wikipedia.org/wiki/Brainfuck) interpreter
//! [`BrainfuckInterp`] and a jit compiler [`BrainfuckJit`].
//!
//! Brainfuck is an esoteric programming languge existing of 8 commands.
//! - `>` increment data pointer.
//! - `<` decrement data pointer.
//! - `+` increment data at current data pointer.
//! - `-` decrement data at current data pointer.
//! - `.` output data at current data pointer.
//! - `,` read input and store at current data pointer.
//! - `[` jump behind matching ']' if data at data pointer is zero.
//! - `]` jump behind matching '[' if data at data pointer is non-zero.

use std::collections::HashMap;
use std::io::Write;

use juicebox_asm::insn::*;
use juicebox_asm::Runtime;
use juicebox_asm::{Asm, Imm64, Imm8, Label, MemOp, MemOp8, Reg64, Reg8};

// -- BRAINFUCK INTERPRETER ----------------------------------------------------

struct BrainfuckInterp {
    pc: usize,
    imem: Vec<char>,
    dptr: usize,
    dmem: [u8; 256],
    branches: HashMap<usize, usize>,
}

impl BrainfuckInterp {
    fn new(prog: &str) -> Result<Self, String> {
        // Do a first pass over the bf program to filter whitespace and detect
        // invalid tokens. Additionally validate all conditional branches, and
        // compute their branch target.
        let (imem, branches) = {
            // Instruction memory holding the final bf program.
            let mut imem = Vec::new();
            // Helper to track index of open brackets.
            let mut lhs_brackets = Vec::new();
            // Mapping from branch instruction to branch target.
            let mut branches = HashMap::new();

            for (idx, token) in prog.chars().filter(|c| !c.is_whitespace()).enumerate() {
                match token {
                    '<' | '>' | '+' | '-' | '.' | ',' => { /* ignore valid bf tokens */ }
                    '[' => lhs_brackets.push(idx),
                    ']' => {
                        if let Some(lhs) = lhs_brackets.pop() {
                            branches.insert(lhs, idx);
                            branches.insert(idx, lhs);
                        } else {
                            return Err(format!("encountered un-balanced brackets, found ']' at index {idx} without matching '['"));
                        }
                    }
                    _ => return Err(format!("invalid bf token '{token}'")),
                }
                imem.push(token)
            }

            if !lhs_brackets.is_empty() {
                return Err(String::from(
                    "encountered un-balanced brackets, left-over '[' after parsing bf program",
                ));
            }

            (imem, branches)
        };

        Ok(BrainfuckInterp {
            pc: 0,
            imem,
            dptr: 0,
            dmem: [0; 256],
            branches,
        })
    }
}

fn run_interp(prog: &str) {
    let mut vm = BrainfuckInterp::new(prog).unwrap();

    loop {
        let insn = match vm.imem.get(vm.pc) {
            Some(insn) => insn,
            None => break, // End of bf program.
        };

        let putchar = |val: u8| {
            std::io::stdout()
                .write(&[val])
                .expect("Failed to write to stdout!");
        };

        match insn {
            '>' => {
                vm.dptr += 1;
                assert!(vm.dptr < vm.dmem.len());
            }
            '<' => {
                assert!(vm.dptr > 0);
                vm.dptr -= 1;
            }
            '+' => {
                vm.dmem[vm.dptr] += 1;
            }
            '-' => {
                vm.dmem[vm.dptr] -= 1;
            }
            '.' => {
                putchar(vm.dmem[vm.dptr]);
            }
            ',' => {
                unimplemented!("getchar");
            }
            '[' => {
                if vm.dmem[vm.dptr] == 0 {
                    vm.pc = *vm.branches.get(&vm.pc).unwrap();
                }
            }
            ']' => {
                if vm.dmem[vm.dptr] != 0 {
                    vm.pc = *vm.branches.get(&vm.pc).unwrap();
                }
            }
            _ => unreachable!(),
        }

        vm.pc += 1;
    }
}

// -- BRAINFUCK JIT ------------------------------------------------------------

#[cfg(not(any(target_arch = "x86_64", target_os = "linux")))]
compile_error!("Only supported on x86_64 with SystemV abi");

struct BrainfuckJit {
    imem: Vec<char>,
    dmem: [u8; 256],
}

impl BrainfuckJit {
    fn new(prog: &str) -> Result<Self, String> {
        // Do a first pass over the bf program to filter whitespace and detect
        // invalid tokens.
        let imem = prog
            .chars()
            .filter(|c| !c.is_whitespace())
            .map(|c| match c {
                '<' | '>' | '+' | '-' | '.' | ',' | '[' | ']' => Ok(c),
                _ => Err(format!("invalid bf token '{c}'")),
            })
            .collect::<Result<Vec<char>, String>>()?;

        Ok(BrainfuckJit {
            imem,
            dmem: [0; 256],
        })
    }
}

extern "C" fn putchar(c: u8) {
    std::io::stdout()
        .write(&[c])
        .expect("Failed to write to stdout!");
}

fn run_jit(prog: &str) {
    let mut vm = BrainfuckJit::new(prog).unwrap();

    // Use callee saved registers to hold vm state, such that we don't need to
    // save any state before calling out to putchar.
    let dmem_base = Reg64::rbx;
    let dmem_idx = Reg64::r12;

    let mut asm = Asm::new();
    // Move data memory pointer (argument on jit entry) into correct register.
    asm.mov(dmem_base, Reg64::rdi);
    // Clear data memory index.
    asm.xor(dmem_idx, dmem_idx);

    // A stack of label pairs, used to link up forward and backward jumps for a
    // given '[]' pair.
    let mut label_stack = Vec::new();

    // Generate code for each instruction in the bf program.
    let mut pc = 0;
    while pc < vm.imem.len() {
        match vm.imem[pc] {
            '>' => {
                // TODO: generate runtime bounds check.
                asm.inc(dmem_idx);
            }
            '<' => {
                // TODO: generate runtime bounds check.
                asm.dec(dmem_idx);
            }
            '+' => {
                // Apply optimization to fold consecutive '+' instructions to a
                // single add instruction during compile time.

                match vm.imem[pc..].iter().take_while(|&&i| i.eq(&'+')).count() {
                    1 => asm.inc(MemOp8::from(MemOp::IndirectBaseIndex(dmem_base, dmem_idx))),
                    cnt if cnt <= i8::MAX as usize => {
                        // For add m64, imm8, the immediate is sign-extend and
                        // hence treated as signed.
                        asm.add(
                            MemOp::IndirectBaseIndex(dmem_base, dmem_idx),
                            Imm8::from(cnt as u8),
                        );

                        // Advance pc, but account for pc increment at the end
                        // of the loop.
                        pc += cnt - 1;
                    }
                    cnt @ _ => unimplemented!("cnt={cnt} oob, add with larger imm"),
                }
            }
            '-' => {
                // Apply optimization to fold consecutive '-' instructions to a
                // single sub instruction during compile time.

                match vm.imem[pc..].iter().take_while(|&&i| i.eq(&'-')).count() {
                    1 => asm.dec(MemOp8::from(MemOp::IndirectBaseIndex(dmem_base, dmem_idx))),
                    cnt if cnt <= i8::MAX as usize => {
                        // For sub m64, imm8, the immediate is sign-extend and
                        // hence treated as signed.
                        asm.sub(
                            MemOp::IndirectBaseIndex(dmem_base, dmem_idx),
                            Imm8::from(cnt as u8),
                        );

                        // Advance pc, but account for pc increment at the end
                        // of the loop.
                        pc += cnt - 1;
                    }
                    cnt @ _ => unimplemented!("cnt={cnt} oob, sub with larger imm"),
                }
            }
            '.' => {
                // Load data memory from active cell into di register, which is
                // the first argument register according to the SystemV abi,
                // then call into putchar. Since we stored all out vm state in
                // callee saved registers we don't need to save any registers
                // before the call.
                asm.mov(Reg8::dil, MemOp::IndirectBaseIndex(dmem_base, dmem_idx));
                asm.mov(Reg64::rax, Imm64::from(putchar as usize));
                asm.call(Reg64::rax);
            }
            ',' => {
                unimplemented!("getchar");
            }
            '[' => {
                // Create new label pair.
                label_stack.push((Label::new(), Label::new()));
                // UNWRAP: We just pushed a new entry on the stack.
                let label_pair = label_stack.last_mut().unwrap();

                // Goto label_pair.0 if data memory at active cell is 0.
                //   if vm.dmem[vm.dptr] == 0 goto label_pair.0
                asm.cmp(
                    MemOp::IndirectBaseIndex(dmem_base, dmem_idx),
                    Imm8::from(0u8),
                );
                asm.jz(&mut label_pair.0);

                // Bind label_pair.1 after the jump instruction, which will be
                // the branch target for the matching ']'.
                asm.bind(&mut label_pair.1);
            }
            ']' => {
                let mut label_pair = label_stack
                    .pop()
                    .expect("encountered un-balanced brackets, found ']' without matching '['");

                // Goto label_pair.1 if data memory at active cell is not 0.
                //   if vm.dmem[vm.dptr] != 0 goto label_pair.1
                asm.cmp(
                    MemOp::IndirectBaseIndex(dmem_base, dmem_idx),
                    Imm8::from(0u8),
                );
                asm.jnz(&mut label_pair.1);

                // Bind label_pair.0 after the jump instruction, which is the
                // branch target for the matching '['.
                asm.bind(&mut label_pair.0);
            }
            _ => unreachable!(),
        }

        // Increment pc to next instruction.
        pc += 1;
    }

    // Return from bf program.
    asm.ret();

    if !label_stack.is_empty() {
        panic!("encountered un-balanced brackets, left-over '[' after jitting bf program")
    }

    // Execute jitted bf program.
    let mut rt = Runtime::new();
    let bf_entry = unsafe { rt.add_code::<extern "C" fn(*mut u8)>(asm.into_code()) };
    bf_entry(&mut vm.dmem as *mut u8);
}

// -- MAIN ---------------------------------------------------------------------

fn main() {
    // https://en.wikipedia.org/wiki/Brainfuck#Adding_two_values
    //let inp = "++>+++++ [<+>-] ++++++++[<++++++>-]<.";
    //println!("add-print-7 (wikipedia.org) - interp");
    //run_interp(inp);
    //println!("add-print-7 (wikipedia.org) - jit");
    //run_jit(inp);

    // https://en.wikipedia.org/wiki/Brainfuck#Hello_World!
    let inp = "++++++++[>++++[>++>+++>+++>+<<<<-]>+>+>->>+[<]<-]>>.>---.+++++++..+++.>>.<-.<.+++.------.--------.>>+.>++.";
    println!("hello-world (wikipedia.org) - interp");
    run_interp(inp);
    println!("hello-world (wikipedia.org) - jit");
    run_jit(inp);

    // https://programmingwiki.de/Brainfuck
    let inp = ">+++++++++[<++++++++>-]<.>+++++++[<++++>-]<+.+++++++..+++.[-]>++++++++[<++++>-] <.>+++++++++++[<++++++++>-]<-.--------.+++.------.--------.[-]>++++++++[<++++>- ]<+.[-]++++++++++.";
    println!("hello-world (programmingwiki.de) - interp");
    run_interp(inp);
    println!("hello-world (programmingwiki.de) - jit");
    run_jit(inp);
}
