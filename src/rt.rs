//! A simple runtime which can be used to execute emitted instructions.

use core::ffi::c_void;
use nix::sys::mman::{mmap, mprotect, munmap, MapFlags, ProtFlags};

/// A simple `mmap`ed runtime with executable pages.
pub struct Runtime {
    buf: *mut c_void,
    len: usize,
}

impl Runtime {
    /// Create a new [Runtime].
    pub fn new(code: impl AsRef<[u8]>) -> Runtime {
        // Allocate a single page.
        let len = core::num::NonZeroUsize::new(4096).unwrap();
        let buf = unsafe {
            mmap(
                None,
                len,
                ProtFlags::PROT_WRITE,
                MapFlags::MAP_PRIVATE | MapFlags::MAP_ANONYMOUS,
                0, /* fd */
                0, /* off */
            )
            .unwrap()
        };
        {
            // Copy over code.
            let code = code.as_ref();
            assert!(code.len() < len.get());
            unsafe { std::ptr::copy_nonoverlapping(code.as_ptr(), buf.cast(), len.get()) };
        }
        unsafe {
            // Remove write permissions from code buffer and allow to read-execute from it.
            mprotect(buf, len.get(), ProtFlags::PROT_READ | ProtFlags::PROT_EXEC)
                .expect("Failed to RX mprotect Runtime code buffer");
        }

        Runtime {
            buf,
            len: len.get(),
        }
    }

    /// Reinterpret the block of code as `F`.
    #[inline]
    pub unsafe fn as_fn<F>(&self) -> F {
        unsafe { std::mem::transmute_copy(&self.buf) }
    }
}

impl Drop for Runtime {
    fn drop(&mut self) {
        unsafe {
            munmap(self.buf, self.len).expect("Failed to munmap Runtime");
        }
    }
}
