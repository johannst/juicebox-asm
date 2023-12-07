//! Simple `mmap`ed runtime.
//!
//! This runtime supports adding code to executable pages and turn the added code into user
//! specified function pointer.

use nix::sys::mman::{mmap, mprotect, munmap, MapFlags, ProtFlags};

#[cfg(not(target_os = "linux"))]
compile_error!("This runtime is only supported on linux");

/// A simple `mmap`ed runtime with executable pages.
pub struct Runtime {
    buf: *mut u8,
    len: usize,
    idx: usize,
}

impl Runtime {
    /// Create a new [Runtime].
    ///
    /// # Panics
    ///
    /// Panics if the `mmap` call fails.
    pub fn new() -> Runtime {
        // Allocate a single page.
        let len = core::num::NonZeroUsize::new(4096).expect("Value is non zero");
        let buf = unsafe {
            mmap(
                None,
                len,
                ProtFlags::PROT_NONE,
                MapFlags::MAP_PRIVATE | MapFlags::MAP_ANONYMOUS,
                0, /* fd */
                0, /* off */
            )
            .expect("Failed to mmap runtime code page") as *mut u8
        };

        Runtime {
            buf,
            len: len.get(),
            idx: 0,
        }
    }

    /// Add the block of `code` to the runtime and a get function pointer of type `F`.
    ///
    /// # Panics
    ///
    /// Panics if the `code` does not fit on the `mmap`ed pages or is empty.
    ///
    /// # Safety
    ///
    /// The code added must fulfill the ABI of the specified function `F` and the returned function
    /// pointer is only valid until the [`Runtime`] is dropped.
    ///
    /// # Examples
    ///
    /// ```
    /// let mut rt = juicebox_asm::Runtime::new();
    ///
    /// let code = [ 0x90 /* nop */, 0xc3 /* ret */ ];
    /// let nop = unsafe { rt.add_code::<extern "C" fn()>(&code) };
    ///
    /// nop();
    /// ```
    pub unsafe fn add_code<F>(&mut self, code: impl AsRef<[u8]>) -> F {
        // Get pointer to start of next free byte.
        assert!(self.idx < self.len, "Runtime code page full");
        let fn_start = self.buf.add(self.idx);

        // Copy over code.
        let code = code.as_ref();
        assert!(!code.is_empty(), "Adding empty code not supported");
        assert!(
            code.len() <= (self.len - self.idx),
            "Code does not fit on the runtime code page"
        );
        self.unprotect();
        unsafe { std::ptr::copy_nonoverlapping(code.as_ptr(), fn_start, code.len()) };
        self.protect();

        // Increment index to next free byte.
        self.idx += code.len();

        // Return function to newly added code.
        unsafe { Self::as_fn::<F>(fn_start) }
    }

    /// Dump the code added so far to the runtime into a file called `jit.asm` in the processes
    /// current working directory.
    ///
    /// The code can be inspected with a disassembler as for example `ndiasm` from
    /// [nasm.us](https://nasm.us/index.php).
    /// ```sh
    /// ndisasm -b 64 jit.asm
    /// ```
    ///
    /// # Panics
    ///
    /// Panics if writing the file failed.
    pub fn dump(&self) {
        assert!(self.idx <= self.len);
        let code = unsafe { core::slice::from_raw_parts(self.buf, self.idx) };
        std::fs::write("jit.asm", code).expect("Failed to write file");
    }

    /// Reinterpret the block of code pointed to by `fn_start` as `F`.
    #[inline]
    unsafe fn as_fn<F>(fn_start: *mut u8) -> F {
        unsafe { std::mem::transmute_copy(&fn_start) }
    }

    /// Add write protection the underlying code page(s).
    ///
    /// # Panics
    ///
    /// Panics if the `mprotect` call fails.
    fn protect(&mut self) {
        unsafe {
            // Remove write permissions from code page and allow to read-execute from it.
            mprotect(
                self.buf.cast(),
                self.len,
                ProtFlags::PROT_READ | ProtFlags::PROT_EXEC,
            )
            .expect("Failed to RX mprotect runtime code page");
        }
    }

    /// Remove write protection the underlying code page(s).
    ///
    /// # Panics
    ///
    /// Panics if the `mprotect` call fails.
    fn unprotect(&mut self) {
        unsafe {
            // Add write permissions to code page.
            mprotect(self.buf.cast(), self.len, ProtFlags::PROT_WRITE)
                .expect("Failed to W mprotect runtime code page");
        }
    }
}

impl Drop for Runtime {
    /// Unmaps the code page. This invalidates all the function pointer returned by
    /// [`Runtime::add_code`].
    fn drop(&mut self) {
        unsafe {
            munmap(self.buf.cast(), self.len).expect("Failed to munmap runtime");
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_code_max_size() {
        let mut rt = Runtime::new();
        let code = [0u8; 4096];
        unsafe {
            rt.add_code::<extern "C" fn()>(code);
        }
    }

    #[test]
    #[should_panic]
    fn test_code_max_size_plus_1() {
        let mut rt = Runtime::new();
        let code = [0u8; 4097];
        unsafe {
            rt.add_code::<extern "C" fn()>(code);
        }
    }

    #[test]
    #[should_panic]
    fn test_code_max_size_plus_1_2() {
        let mut rt = Runtime::new();
        let code = [0u8; 4096];
        unsafe {
            rt.add_code::<extern "C" fn()>(code);
        }

        let code = [0u8; 1];
        unsafe {
            rt.add_code::<extern "C" fn()>(code);
        }
    }

    #[test]
    #[should_panic]
    fn test_empty_code() {
        let mut rt = Runtime::new();
        let code = [0u8; 0];
        unsafe {
            rt.add_code::<extern "C" fn()>(code);
        }
    }
}
