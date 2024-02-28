//! Simple `mmap`ed runtime.
//!
//! This runtime supports adding code to executable pages and turn the added code into user
//! specified function pointer.

#[cfg(not(target_os = "linux"))]
compile_error!("This runtime is only supported on linux");

use nix::sys::mman::{mmap, mprotect, munmap, MapFlags, ProtFlags};

mod perf {
    use std::fs;
    use std::io::Write;

    /// Provide support for the simple [perf jit interface][perf-jit].
    ///
    /// This allows a simple (static) jit runtime to generate meta data describing the generated
    /// functions, which is used during post-processing by `perf report` to symbolize addresses
    /// captured while executing jitted code.
    ///
    /// By the nature of this format, this can not be used for dynamic jit runtimes, which reuses
    /// memory which previously contained jitted code.
    ///
    /// [perf-jit]: https://elixir.bootlin.com/linux/v6.6.6/source/tools/perf/Documentation/jit-interface.txt
    pub(super) struct PerfMap {
        file: std::fs::File,
    }

    impl PerfMap {
        /// Create an empty perf map file.
        pub(super) fn new() -> Self {
            let name = format!("/tmp/perf-{}.map", nix::unistd::getpid());
            let file = fs::OpenOptions::new()
                .truncate(true)
                .create(true)
                .write(true)
                .open(&name)
                .unwrap_or_else(|_| panic!("Failed to open perf map file {}", &name));

            PerfMap { file }
        }

        /// Add an entry to the perf map file.
        pub(super) fn add_entry(&mut self, start: usize, len: usize) {
            // Each line has the following format, fields separated with spaces:
            //   START SIZE NAME
            //
            // START and SIZE are hex numbers without 0x.
            // NAME is the rest of the line, so it could contain special characters.
            writeln!(self.file, "{:x} {:x} jitfn_{:x}", start, len, start)
                .expect("Failed to write PerfMap entry");
        }
    }
}

/// A simple `mmap`ed runtime with executable pages.
pub struct Runtime {
    buf: *mut u8,
    len: usize,
    idx: usize,
    perf: Option<perf::PerfMap>,
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
            perf: None,
        }
    }

    /// Create a new [Runtime] which also generates static perf metat data.
    ///
    /// For each function added to the [Runtime], an entry will be generated in the
    /// `/tmp/perf-<PID>.map` file, which `perf report` uses to symbolicate unknown addresses.
    /// This is applicable for static runtimes only.
    ///
    /// # Panics
    ///
    /// Panics if the `mmap` call fails.
    pub fn with_profile() -> Runtime {
        let mut rt = Runtime::new();
        rt.perf = Some(perf::PerfMap::new());
        rt
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

        // Add perf map entry.
        if let Some(map) = &mut self.perf {
            map.add_entry(fn_start as usize, code.len());
        }

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
