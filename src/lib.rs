#![cfg_attr(docsrs, feature(doc_auto_cfg))]
#![deny(clippy::undocumented_unsafe_blocks)]

use std::cell::RefCell;
use std::sync::{Arc, OnceLock};
use std::{error, fmt, io};

#[cfg(unix)]
mod unix;
use parking_lot::{ReentrantMutex, ReentrantMutexGuard};
#[cfg(unix)]
use unix as imp;
#[cfg(windows)]
mod windows;
#[cfg(windows)]
use windows as imp;

mod io_error;

static TERMINAL: OnceLock<Result<ReentrantMutex<RefCell<imp::Terminal>>, Arc<io::Error>>> =
    OnceLock::new();

/// Creates a readable and writable handle to the terminal (or TTY) if available.
///
/// Each handle is a reference to a shared instance whose access is synchronized via a mutex.
/// Use [`Terminal::lock`] if you want to avoid locking before each read / write call.
///
/// ## Unix
/// On Unix, the terminal is retrieved by successively testing
/// * the standard error,
/// * standard input,
/// * standard output,
/// * and finally `/dev/tty`.
///
/// ## Windows
/// On Windows, the reading half is retrieved by first testing the standard input falling back to `CONIN$`. \
/// The writing half is retrieved by successfully testing
/// * the standard error,
/// * standard output,
/// * and finally `CONOUT$`.
pub fn terminal() -> Result<Terminal, io::Error> {
    TERMINAL
        .get_or_init(|| {
            imp::terminal()
                .map_err(Arc::new)
                .map(|t| ReentrantMutex::new(RefCell::new(t)))
        })
        .as_ref()
        .map(Terminal)
        .map_err(io_error::shared_io_error)
}

/// A readable and writable handle to the terminal (or TTY).
/// Created using [`terminal()`].
#[derive(Debug)]
pub struct Terminal(&'static ReentrantMutex<RefCell<imp::Terminal>>);

#[cfg(test)]
static_assertions::assert_impl_all!(Terminal: Send, Sync);

impl io::Read for Terminal {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.0.lock().borrow_mut().read(buf)
    }
}

impl io::Write for Terminal {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.0.lock().borrow_mut().write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.0.lock().borrow_mut().flush()
    }
}

impl Terminal {
    /// Locks access to this terminal, returing a guard that is readable and writable.
    ///
    /// Until the returned [`TerminalLock`] is dropped, all standard I/O streams
    /// that refer to the same terminal will be locked.
    pub fn lock(&mut self) -> TerminalLock<'_> {
        let inner = self.0.lock();
        let stdio_locks = inner.borrow().lock_stdio();
        TerminalLock { stdio_locks, inner }
    }
}

#[derive(Debug)]
pub struct TerminalLock<'a> {
    inner: ReentrantMutexGuard<'a, RefCell<imp::Terminal>>,
    #[allow(dead_code)]
    stdio_locks: StdioLocks,
}

#[cfg(test)]
static_assertions::assert_not_impl_any!(TerminalLock<'_>: Send, Sync);

impl<'a> io::Read for TerminalLock<'a> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.inner.borrow_mut().read(buf)
    }
}

impl<'a> io::Write for TerminalLock<'a> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.inner.borrow_mut().write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.inner.borrow_mut().flush()
    }
}

#[derive(Debug)]
struct StdioLocks {
    #[allow(dead_code)]
    stdin_lock: Option<io::StdinLock<'static>>,
    #[allow(dead_code)]
    stdout_lock: Option<io::StdoutLock<'static>>,
    #[allow(dead_code)]
    stderr_lock: Option<io::StderrLock<'static>>,
}
