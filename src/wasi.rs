//! # sys (WASI)
//!
//! WASI-specific structs and functions. Will be imported as `sys` on WASI
//! systems.

use std::{
    fs::File,
    io,
    sync::atomic::{AtomicBool, Ordering},
};

use crate::Error;
pub use crate::xdg::*;

/// METAL's terminal interface (`metal:os/terminal`): the window's size, and waiting for keys or
/// a resize (WASI 0.2 has neither a window-size call nor `SIGWINCH`).
#[allow(unsafe_code, clippy::all, clippy::pedantic, missing_docs, unreachable_pub, unused_qualifications)]
mod metal {
    wit_bindgen::generate!({
        inline: "
            package metal:os@0.1.0;
            interface terminal {
                enum event { input, resized, timeout }
                size: func() -> option<tuple<u16, u16>>;
                wait: func(timeout-ms: option<u32>) -> event;
            }
            world kibi { import terminal; }
        ",
    });
}

use metal::metal::os::terminal;

/// The terminal said the size changed (and kibi hasn't looked yet).
static RESIZED: AtomicBool = AtomicBool::new(false);

#[derive(Copy, Clone)]
pub struct TermMode;

/// Return the current window size as (rows, columns), from the `LINES` and
/// `COLUMNS` environment variables (WASI 0.2 has no call for it; METAL sets
/// them). Without them, return an error, so that kibi falls back to another
/// method of getting the window size.
pub fn get_window_size() -> Result<(usize, usize), Error> {
    if let Some((cols, rows)) = terminal::size() {
        return Ok((usize::from(rows), usize::from(cols)));
    }
    let var = |name| std::env::var(name).ok()?.parse::<usize>().ok().filter(|n| *n > 0);
    match (var("LINES"), var("COLUMNS")) {
        (Some(rows), Some(cols)) => Ok((rows, cols)),
        _ => Err(Error::InvalidWindowSize),
    }
}

/// Register a signal handler that sets a global variable when the window size
/// changes. On WASI platforms, this does nothing.
#[expect(clippy::unnecessary_wraps)] // Result required on other platforms
pub const fn register_winsize_change_signal_handler() -> io::Result<()> { Ok(()) }

/// Check if the windows size has changed since the last call to this function
/// (METAL's terminal reports resizes while kibi waits for keys).
pub fn has_window_size_changed() -> bool { RESIZED.swap(false, Ordering::Relaxed) }

/// Stdin at METAL's terminal: waits for keys or a resize. A resize reads as 0
/// bytes (kibi's input loop then checks the window size, as it does when a
/// read times out on Unix).
struct TerminalStdin;

impl io::Read for TerminalStdin {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        loop {
            match terminal::wait(None) {
                terminal::Event::Input => return io::stdin().read(buf),
                terminal::Event::Resized => {
                    RESIZED.store(true, Ordering::Relaxed);
                    return Ok(0);
                }
                terminal::Event::Timeout => {}
            }
        }
    }
}

/// Set the terminal mode. On WASI platforms, this does nothing.
#[expect(clippy::unnecessary_wraps)] // Result required on other platforms
#[expect(clippy::trivially_copy_pass_by_ref)]
pub const fn set_term_mode(_term: &TermMode) -> io::Result<()> { Ok(()) }

// Opening the file /dev/tty is effectively the same as `raw_mode`
#[expect(clippy::unnecessary_wraps)] // Result required on other platforms
pub const fn enable_raw_mode() -> io::Result<TermMode> { Ok(TermMode {}) }

/// Construct a new handle to the standard input of the current process: the
/// terminal when the host gives stdin a terminal (`wasi:cli/terminal-stdin`,
/// as METAL does), else `/dev/tty` (e.g. `wasmtime --dir /dev`).
///
/// # Errors
///
/// This function will return an error if stdin isn't a terminal and /dev/tty
/// cannot be open.
pub fn stdin() -> io::Result<Box<dyn io::BufRead>> {
    use std::io::IsTerminal as _;
    if io::stdin().is_terminal() {
        return Ok(Box::new(io::BufReader::new(TerminalStdin)));
    }
    Ok(Box::new(io::BufReader::new(File::open("/dev/tty")?)))
}

pub fn path(filename: &str) -> std::path::PathBuf {
    // If the filename is absolute then it starts with a forward slash and we
    // can just open the file however if it lacks a forward slash then its
    // relative to the current working directory. As WASI does not have an ABI
    // for current directory we are using the PWD environment variable as a
    // defacto standard
    if filename.starts_with('/') {
        std::path::PathBuf::from(filename)
    } else {
        std::env::current_dir().unwrap_or_else(|_| "/".into()).join(filename)
    }
}
