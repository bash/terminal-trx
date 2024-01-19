use self::console_mode::{
    enable_raw_mode, get_console_mode, is_raw_mode_enabled, set_console_mode,
};
use crate::{StdioLocks, TransceiveExt};
use msys::msys_tty_on;
use std::fs::{File, OpenOptions};
use std::io::{self, IsTerminal};
use std::mem::ManuallyDrop;
use std::os::windows::io::{AsHandle, AsRawHandle, BorrowedHandle, FromRawHandle, RawHandle};
use thiserror::Error;
use windows_sys::Win32::Foundation::BOOL;
use windows_sys::Win32::System::Console::CONSOLE_MODE;

mod console_mode;
mod msys;

pub(crate) fn terminal() -> io::Result<Terminal> {
    // TODO: Track which standard I/O handles are the same.
    Ok(Terminal {
        conin: conin()?,
        conout: conout()?,
    })
}

fn conin() -> io::Result<ConsoleBuffer> {
    ConsoleBuffer::try_borrow(io::stdin())
        .map(Ok)
        .unwrap_or_else(|| {
            OpenOptions::new()
                .read(true)
                .open("CONIN$")
                .map(ConsoleBuffer::Owned)
        })
}

fn conout() -> io::Result<ConsoleBuffer> {
    ConsoleBuffer::try_borrow(io::stderr())
        .or_else(|| ConsoleBuffer::try_borrow(io::stdout()))
        .map(Ok)
        .unwrap_or_else(|| {
            OpenOptions::new()
                .write(true)
                .open("CONOUT$")
                .map(ConsoleBuffer::Owned)
        })
}

#[derive(Debug)]
pub(crate) struct Terminal {
    conin: ConsoleBuffer,
    conout: ConsoleBuffer,
}

#[derive(Debug)]
pub(crate) enum ConsoleBuffer {
    Owned(File),
    Borrowed(ManuallyDrop<File>),
}

impl ConsoleBuffer {
    // SAFETY: Only pass handles to global standard I/O that lives for the entire duration of the program.
    fn try_borrow(handle: impl AsHandle) -> Option<ConsoleBuffer> {
        let handle = handle.as_handle();
        handle.is_terminal().then(|| {
            // SAFETY: We pass a valid handle and we ensure that the
            // standard I/O handle is not closed by wrapping the file in `ManuallyDrop`.
            ConsoleBuffer::Borrowed(ManuallyDrop::new(unsafe {
                File::from_raw_handle(handle.as_raw_handle())
            }))
        })
    }
}

impl io::Write for Terminal {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.conout.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.conout.flush()
    }
}

impl AsHandle for ConsoleBuffer {
    fn as_handle(&self) -> BorrowedHandle<'_> {
        match self {
            ConsoleBuffer::Owned(f) => f.as_handle(),
            ConsoleBuffer::Borrowed(f) => f.as_handle(),
        }
    }
}

impl AsRawHandle for ConsoleBuffer {
    fn as_raw_handle(&self) -> RawHandle {
        match self {
            ConsoleBuffer::Owned(f) => f.as_raw_handle(),
            ConsoleBuffer::Borrowed(f) => f.as_raw_handle(),
        }
    }
}

impl io::Write for ConsoleBuffer {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        match self {
            ConsoleBuffer::Owned(f) => f.write(buf),
            ConsoleBuffer::Borrowed(f) => f.write(buf),
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        match self {
            ConsoleBuffer::Owned(f) => f.flush(),
            ConsoleBuffer::Borrowed(f) => f.flush(),
        }
    }
}

impl io::Read for Terminal {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.conin.read(buf)
    }
}

impl io::Read for ConsoleBuffer {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        match self {
            ConsoleBuffer::Owned(f) => f.read(buf),
            ConsoleBuffer::Borrowed(f) => f.read(buf),
        }
    }
}

impl Terminal {
    pub(crate) fn lock_stdio(&mut self) -> StdioLocks {
        StdioLocks {
            stdin_lock: None,
            stdout_lock: None,
            stderr_lock: None,
        }
    }

    pub(crate) fn enable_raw_mode(&mut self) -> io::Result<RawModeGuard<'_>> {
        let conin = self.conin.as_handle();

        // `is_terminal` recognizes MSYS/Cygwin pipes as terminal,
        // but they are not a console, so we bail out.
        // SAFETY: We pass a valid handle.
        if unsafe { msys_tty_on(conin.as_raw_handle()) } {
            return Err(io::Error::new(
                io::ErrorKind::Unsupported,
                MsysUnsupportedError,
            ));
        }

        let old_mode = set_raw_mode_if_necessary(conin)?;
        Ok(RawModeGuard {
            inner: self,
            old_mode,
        })
    }
}

fn set_raw_mode_if_necessary(handle: BorrowedHandle) -> io::Result<Option<CONSOLE_MODE>> {
    let mode = get_console_mode(handle)?;
    if is_raw_mode_enabled(mode) {
        Ok(None)
    } else {
        set_console_mode(handle, enable_raw_mode(mode))?;
        Ok(Some(mode))
    }
}

#[derive(Debug, Error)]
#[error("enabling raw mode on a MSYS/Cygwin terminal is not supported")]
struct MsysUnsupportedError;

#[derive(Debug)]
pub(crate) struct RawModeGuard<'a> {
    inner: &'a mut Terminal,
    old_mode: Option<CONSOLE_MODE>,
}

impl Drop for RawModeGuard<'_> {
    fn drop(&mut self) {
        if let Some(old_mode) = self.old_mode {
            _ = set_console_mode(self.inner.conin.as_handle(), old_mode);
        }
    }
}

impl io::Write for RawModeGuard<'_> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.inner.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}

impl io::Read for RawModeGuard<'_> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.inner.read(buf)
    }
}

fn to_io_result(result: BOOL) -> io::Result<()> {
    if result == 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}

impl TransceiveExt for super::Terminal {
    fn input_buffer_handle(&self) -> std::os::windows::io::BorrowedHandle<'_> {
        self.0.conin.as_handle()
    }

    fn screen_buffer_handle(&self) -> std::os::windows::io::BorrowedHandle<'_> {
        self.0.conout.as_handle()
    }
}

impl TransceiveExt for super::TerminalLock<'_> {
    fn input_buffer_handle(&self) -> std::os::windows::io::BorrowedHandle<'_> {
        self.inner.conin.as_handle()
    }

    fn screen_buffer_handle(&self) -> std::os::windows::io::BorrowedHandle<'_> {
        self.inner.conout.as_handle()
    }
}

impl TransceiveExt for super::RawModeGuard<'_> {
    fn input_buffer_handle(&self) -> std::os::windows::io::BorrowedHandle<'_> {
        self.0.inner.conin.as_handle()
    }

    fn screen_buffer_handle(&self) -> std::os::windows::io::BorrowedHandle<'_> {
        self.0.inner.conout.as_handle()
    }
}
