//! Brainfuck VM.
//!
//! This example implements a simple [brainfuck][bf] interpreter
//! [`BrainfuckInterp`] and a jit compiler [`BrainfuckJit`].
//!
//! Brainfuck is an esoteric programming languge existing of 8 commands.
//! - `>` increment data pointer.
//! - `<` decrement data pointer.
//! - `+` increment data at current data pointer.
//! - `-` decrement data at current data pointer.
//! - `.` output data at current data pointer.
//! - `,` read input and store at current data pointer.
//! - `[` jump behind matching `]` if data at data pointer is zero.
//! - `]` jump behind matching `[` if data at data pointer is non-zero.
//!
//! The following is the `hello-world` program from [wikipedia][hw].
//! ```
//! ++++++++[>++++[>++>+++>+++>+<<<<-]>+>+>->>+[<]<-]>>.>---.+++++++..+++.>>.<-.<.+++.------.--------.>>+.>++.
//! ```
//!
//! [bf]: https://en.wikipedia.org/wiki/Brainfuck
//! [hw]: https://en.wikipedia.org/wiki/Brainfuck#Hello_World!

use std::collections::HashMap;
use std::io::Write;

use juicebox_asm::insn::*;
use juicebox_asm::Runtime;
use juicebox_asm::{Asm, Imm64, Imm8, Label, Mem8, Reg64, Reg8};

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
        // compute their branch targets.
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
    let dmem_size = Reg64::r12;
    let dmem_idx = Reg64::r13;

    let mut asm = Asm::new();

    // Save callee saved registers before we tamper them.
    asm.push(dmem_base);
    asm.push(dmem_size);
    asm.push(dmem_idx);

    // Move data memory pointer (argument on jit entry) into correct register.
    asm.mov(dmem_base, Reg64::rdi);
    // Move data memory size (compile time constant) into correct register.
    asm.mov(dmem_size, Imm64::from(vm.dmem.len()));
    // Clear data memory index.
    asm.xor(dmem_idx, dmem_idx);

    // A stack of label pairs, used to link up forward and backward jumps for a
    // given '[]' pair.
    let mut label_stack = Vec::new();

    // Label to jump to when a data pointer overflow is detected.
    let mut oob_ov = Label::new();
    // Label to jump to when a data pointer underflow is detected.
    let mut oob_uv = Label::new();

    // Generate code for each instruction in the bf program.
    let mut pc = 0;
    while pc < vm.imem.len() {
        match vm.imem[pc] {
            '>' => {
                asm.inc(dmem_idx);

                // Check for data pointer overflow and jump to error handler if needed.
                asm.cmp(dmem_idx, dmem_size);
                asm.jz(&mut oob_ov);
            }
            '<' => {
                // Check for data pointer underflow and jump to error handler if needed.
                asm.test(dmem_idx, dmem_idx);
                asm.jz(&mut oob_uv);

                asm.dec(dmem_idx);
            }
            '+' => {
                // Apply optimization to fold consecutive '+' instructions to a
                // single add instruction during compile time.

                match vm.imem[pc..].iter().take_while(|&&i| i.eq(&'+')).count() {
                    1 => {
                        asm.inc(Mem8::indirect_base_index(dmem_base, dmem_idx));
                    }
                    cnt if cnt <= u8::MAX as usize => {
                        asm.add(
                            Mem8::indirect_base_index(dmem_base, dmem_idx),
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
                    1 => {
                        asm.dec(Mem8::indirect_base_index(dmem_base, dmem_idx));
                    }
                    cnt if cnt <= u8::MAX as usize => {
                        asm.sub(
                            Mem8::indirect_base_index(dmem_base, dmem_idx),
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
                asm.mov(Reg8::dil, Mem8::indirect_base_index(dmem_base, dmem_idx));
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
                    Mem8::indirect_base_index(dmem_base, dmem_idx),
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
                    Mem8::indirect_base_index(dmem_base, dmem_idx),
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

    let mut epilogue = Label::new();

    // Successful return from bf program.
    asm.xor(Reg64::rax, Reg64::rax);
    asm.bind(&mut epilogue);
    // Restore callee saved registers before returning from jit.
    asm.pop(dmem_idx);
    asm.pop(dmem_size);
    asm.pop(dmem_base);
    asm.ret();

    // Return because of data pointer overflow.
    asm.bind(&mut oob_ov);
    asm.mov(Reg64::rax, Imm64::from(1));
    asm.jmp(&mut epilogue);

    // Return because of data pointer underflow.
    asm.bind(&mut oob_uv);
    asm.mov(Reg64::rax, Imm64::from(2));
    asm.jmp(&mut epilogue);

    if !label_stack.is_empty() {
        panic!("encountered un-balanced brackets, left-over '[' after jitting bf program")
    }

    // Get function pointer to jitted bf program.
    let mut rt = Runtime::new();
    let bf_entry = unsafe { rt.add_code::<extern "C" fn(*mut u8) -> u64>(asm.into_code()) };

    // Execute jitted bf program.
    match bf_entry(&mut vm.dmem as *mut u8) {
        0 => { /* success */ }
        1 => panic!("oob: data pointer overflow"),
        2 => panic!("oob: data pointer underflow"),
        _ => unreachable!(),
    }
}

// -- MAIN ---------------------------------------------------------------------

fn main() {
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

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn data_ptr_no_overflow() {
        let inp = std::iter::repeat('>').take(255).collect::<String>();
        run_jit(&inp);
    }

    #[test]
    #[should_panic]
    fn data_ptr_overflow() {
        let inp = std::iter::repeat('>').take(255 + 1).collect::<String>();
        run_jit(&inp);
    }

    #[test]
    fn data_ptr_no_underflow() {
        let inp = ">><< ><";
        run_jit(inp);
    }

    #[test]
    #[should_panic]
    fn data_ptr_underflow() {
        let inp = ">><< >< <";
        run_jit(&inp);
    }
}
