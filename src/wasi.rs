//! # sys (WASI)
//!
//! WASI-specific structs and functions. Will be imported as `sys` on WASI
//! systems.

use std::{fs::File, io};

use crate::Error;
pub use crate::xdg::*;

#[derive(Copy, Clone)]
pub struct TermMode;

/// Return the current window size as (rows, columns), from the `LINES` and
/// `COLUMNS` environment variables (WASI 0.2 has no call for it; METAL sets
/// them). Without them, return an error, so that kibi falls back to another
/// method of getting the window size.
pub fn get_window_size() -> Result<(usize, usize), Error> {
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

/// Check if the windows size has changed since the last call to this function.
/// On WASI platforms, this always return false.
pub const fn has_window_size_changed() -> bool { false }

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
        return Ok(Box::new(io::stdin().lock()));
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
