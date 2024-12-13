//! Definition of different addressing modes and memory operande used as input
//! and ouput operands in various instructions.

use crate::Reg64;

#[derive(Clone, Copy)]
pub(crate) enum AddrMode {
    /// An indirect memory operand, eg `mov [rax], rcx`.
    Indirect,
    /// An indirect memory operand with additional displacement, eg `mov [rax + 0x10], rcx`.
    IndirectDisp,
    /// An indirect memory operand in the form base + index, eg `mov [rax + rcx], rdx`.
    IndirectBaseIndex,
}

/// Trait to interact with memory operands.
pub(crate) trait Mem {
    /// Get the addressing mode [`AddrMode`] of the memory operand.
    fn mode(&self) -> AddrMode;

    /// Get the base address register of the memory operand.
    fn base(&self) -> Reg64;

    /// Get the index register of the memory operand.
    fn index(&self) -> Reg64;

    /// Get the displacement of the memory operand.
    fn disp(&self) -> i32;

    /// Check if memory operand is 64 bit.
    fn is_64() -> bool;
}

macro_rules! impl_mem {
    ($(#[$doc:meta] $name:ident)+) => {
        $(
        #[$doc]
        pub struct $name {
            mode: AddrMode,
            base: Reg64,
            index: Reg64,
            disp: i32,
        }

        impl Mem for $name {
            fn mode(&self) -> AddrMode {
                self.mode
            }

            fn base(&self) -> Reg64 {
                self.base
            }

            fn index(&self) -> Reg64 {
                self.index
            }

            fn disp(&self) -> i32 {
                self.disp
            }

            fn is_64() -> bool {
                use std::any::TypeId;
                TypeId::of::<Self>() == TypeId::of::<Mem64>()
            }
        }

        impl $name {
            /// Create a memory operand with `indirect` addressing mode.
            /// For example `mov [rax], rcx`.
            pub fn indirect(base: Reg64) -> Self {
                Self {
                    mode: AddrMode::Indirect,
                    base,
                    index: Reg64::rax, /* zero index */
                    disp: 0,
                }
            }

            /// Create a memory operand with `indirect + displacement`
            /// addressing mode.
            /// For example `mov [rax + 0x10], rcx`.
            pub fn indirect_disp(base: Reg64, disp: i32) -> Self {
                Self {
                    mode: AddrMode::IndirectDisp,
                    base,
                    index: Reg64::rax, /* zero index */
                    disp,
                }
            }

            /// Create a memory operand with `base + index` addressing mode.
            /// For example `mov [rax + rcx], rdx`.
            pub fn indirect_base_index(base: Reg64, index: Reg64) -> Self {
                Self {
                    mode: AddrMode::IndirectBaseIndex,
                    base,
                    index,
                    disp: 0,
                }
            }
        }
        )+
    }
}

impl_mem!(
    /// A memory operand with `byte` size (8 bit).
    Mem8
    /// A memory operand with `word` size (16 bit).
    Mem16
    /// A memory operand with `dword` size (32 bit).
    Mem32
    /// A memory operand with `qword` size (64 bit).
    Mem64
);
