use std::collections::HashSet;

/// A label which is used as target for jump instructions.
///
/// ```rust
/// use juicebox_asm::prelude::*;
///
/// let mut lbl = Label::new();
/// let mut asm = Asm::new();
///
/// // Skip the mov instruction.
/// asm.jmp(&mut lbl);
/// asm.mov(Reg64::rax, Reg64::rax);
/// asm.bind(&mut lbl);
/// ```
///
/// # Panics
///
/// Panics if the label is dropped while not yet bound, or having unresolved relocations.
/// This is mainly a safety-guard to detect wrong usage.
pub struct Label {
    /// Location of the label. Will be set after the label is bound, else None.
    location: Option<usize>,

    /// Offsets that must be patched with the label location.
    offsets: HashSet<usize>,
}

impl Label {
    /// Create a new `unbound` [Label].
    pub fn new() -> Label {
        Label {
            location: None,
            offsets: HashSet::new(),
        }
    }

    /// Bind the label to the `location`.
    pub(crate) fn bind(&mut self, loc: usize) {
        // A label can only be bound once!
        assert!(!self.is_bound());

        self.location = Some(loc);
    }

    /// Record an offset that must be patched with the label location.
    pub(crate) fn record_offset(&mut self, off: usize) {
        self.offsets.insert(off);
    }

    pub(crate) fn location(&self) -> Option<usize> {
        self.location
    }

    pub(crate) fn offsets_mut(&mut self) -> &mut HashSet<usize> {
        &mut self.offsets
    }

    /// Check whether the label is bound to a location.
    const fn is_bound(&self) -> bool {
        self.location.is_some()
    }
}

impl Drop for Label {
    fn drop(&mut self) {
        // Ensure the label was bound when it is dropped.
        assert!(self.is_bound());
        // Ensure all offsets have been patched when the label is dropped.
        assert!(self.offsets.is_empty());
    }
}
