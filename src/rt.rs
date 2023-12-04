//! A simple runtime which can be used to execute emitted instructions.

use core::slice;
use nix::sys::mman::{mmap, mprotect, munmap, MapFlags, ProtFlags};

/// A simple `mmap`ed runtime with executable pages.
pub struct Runtime {
    buf: *mut u8,
    len: usize,
    idx: usize,
}

impl Runtime {
    /// Create a new [Runtime].
    pub fn new() -> Runtime {
        // Allocate a single page.
        let len = core::num::NonZeroUsize::new(4096).unwrap();
        let buf = unsafe {
            mmap(
                None,
                len,
                ProtFlags::PROT_WRITE | ProtFlags::PROT_READ | ProtFlags::PROT_EXEC,
                MapFlags::MAP_PRIVATE | MapFlags::MAP_ANONYMOUS,
                0, /* fd */
                0, /* off */
            )
            .unwrap() as *mut u8
        };

        Runtime {
            buf,
            len: len.get(),
            idx: 0,
        }
    }

    /// Write protect the underlying code page(s).
    pub fn protect(&mut self) {
        unsafe {
            // Remove write permissions from code buffer and allow to read-execute from it.
            mprotect(
                self.buf.cast(),
                self.len,
                ProtFlags::PROT_READ | ProtFlags::PROT_EXEC,
            )
            .expect("Failed to RX mprotect Runtime code buffer");
        }
    }

    /// Add block of code to the runtime and get function pointer back.
    pub unsafe fn add_code<F>(&mut self, code: impl AsRef<[u8]>) -> F {
        // Get pointer to start of next free byte.
        assert!(self.idx < self.len);
        let fn_start = self.buf.add(self.idx);

        // Copy over code.
        let code = code.as_ref();
        assert!(code.len() < (self.len - self.idx));
        unsafe { std::ptr::copy_nonoverlapping(code.as_ptr(), fn_start, code.len()) };

        // Increment index to next free byte.
        self.idx += code.len();

        // Return function to newly added code.
        Self::as_fn::<F>(fn_start)
    }

    /// Reinterpret the block of code as `F`.
    #[inline]
    unsafe fn as_fn<F>(fn_start: *mut u8) -> F {
        unsafe { std::mem::transmute_copy(&fn_start) }
    }

    /// Dump the currently added code to a file called `jit.asm`. The disassembly can be inspected
    /// as `ndisasm -b 64 jit.asm`.
    pub fn dump(&self) {
        let code = unsafe { slice::from_raw_parts(self.buf, self.idx) };
        std::fs::write("jit.asm", code).unwrap();
    }
}

impl Drop for Runtime {
    fn drop(&mut self) {
        unsafe {
            munmap(self.buf.cast(), self.len).expect("Failed to munmap Runtime");
        }
    }
}
