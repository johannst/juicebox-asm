use std::io::{ErrorKind, Write};
use std::process::{Command, Stdio};

/// Disassemble the code currently added to the runtime, using
/// [`ndisasm`](https://nasm.us/index.php) and print it to _stdout_. If
/// `ndisasm` is not available on the system this prints a warning and
/// becomes a nop.
///
/// # Panics
///
/// Panics if anything goes wrong with spawning, writing to or reading from
/// the `ndisasm` child process.
pub(crate) fn disasm<T: AsRef<[u8]>>(code: T) {
    let code = code.as_ref();

    // Create ndisasm process, which expects input on stdin.
    let mut child = match Command::new("ndisasm")
        .args(["-b64", "-"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
    {
        Ok(child) => child,
        Err(err) if err.kind() == ErrorKind::NotFound => {
            println!("disasm: skipping, ndisasm not found");
            return;
        }
        Err(err) => {
            panic!("{:?}", err);
        }
    };

    // Write code to stdin of ndisasm.
    child
        .stdin
        .take()
        .expect("failed to take stdin")
        .write_all(code)
        .expect("failed to write bytes to stdin");

    // Wait for output from ndisasm and print to stdout.
    println!(
        "{}",
        String::from_utf8_lossy(
            &child
                .wait_with_output()
                .expect("failed to get stdout")
                .stdout
        )
    );
}
