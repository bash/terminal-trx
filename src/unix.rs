use crate::StdioLocks;
use libc::{c_int, fcntl, termios, F_GETFL, O_RDWR};
use std::ffi::{CStr, CString, OsStr};
use std::fmt;
use std::fs::{File, OpenOptions};
use std::io::{self, stderr, stdin, stdout, IsTerminal};
use std::mem::{self, ManuallyDrop};
use std::ops::{Deref, DerefMut};
use std::os::fd::{AsFd, AsRawFd, BorrowedFd, FromRawFd as _};
use std::os::unix::ffi::OsStrExt;

mod attr;

pub(crate) fn terminal() -> io::Result<Terminal> {
    None.or_else(|| reuse_tty_from_stdio(stderr).transpose())
        .or_else(|| reuse_tty_from_stdio(stdout).transpose())
        .or_else(|| reuse_tty_from_stdio(stdin).transpose())
        .map(|r| r.and_then(Terminal::from_stdio))
        .unwrap_or_else(|| Ok(Terminal::from_controlling(open_controlling_tty()?)))
}

fn reuse_tty_from_stdio<S: IsTerminal + AsFd>(
    stream: impl FnOnce() -> S,
) -> io::Result<Option<TerminalFile>> {
    let stream = stream();

    if stream.is_terminal() {
        // This branch here is a bit questionable to me:
        // I've seen a lot of code that re-uses the standard I/O fd if possible.
        // But I don't quite understand what the benefit of that is. Is it to have as little fds open as possible?
        // Is it a lot faster than opening the tty ourselves?
        if is_read_write(stream.as_fd())? {
            // SAFETY: We know that the file descriptor is valid.
            // However we break the assumption that the file descriptor is owned.
            // That's why the file is immediately wrapped in a ManuallyDrop to prevent
            // the standard I/O descriptor from being closed.
            let file = unsafe { File::from_raw_fd(stream.as_fd().as_raw_fd()) };
            Ok(Some(TerminalFile::Borrowed(ManuallyDrop::new(file))))
        } else {
            reopen_tty(stream.as_fd())
                .map(TerminalFile::Owned)
                .map(Some)
        }
    } else {
        Ok(None)
    }
}

fn open_controlling_tty() -> io::Result<TerminalFile> {
    OpenOptions::new()
        .read(true)
        .write(true)
        .open("/dev/tty")
        .map(TerminalFile::Owned)
}

fn is_read_write(fd: BorrowedFd) -> io::Result<bool> {
    // SAFETY: We know that the file descriptor is valid.
    let mode = to_io_result(unsafe { fcntl(fd.as_raw_fd(), F_GETFL) })?;
    Ok(mode & O_RDWR == O_RDWR)
}

fn reopen_tty(fd: BorrowedFd) -> io::Result<File> {
    let name = ptsname_r(fd)?;
    OpenOptions::new()
        .read(true)
        .write(true)
        .open(OsStr::from_bytes(name.as_bytes()))
}

fn is_same_file(a: BorrowedFd, b: BorrowedFd) -> io::Result<bool> {
    Ok(a.as_raw_fd() == b.as_raw_fd() || {
        let stat_a = fstat(a)?;
        let stat_b = fstat(b)?;
        stat_a.st_dev == stat_b.st_dev && stat_a.st_ino == stat_b.st_ino
    })
}

fn fstat(fd: BorrowedFd) -> io::Result<libc::stat> {
    // SAFETY: If fstat is successful, then we get a valid stat structure.
    let mut stat = unsafe { mem::zeroed() };
    // SAFETY: We know that the file descriptor is valid.
    to_io_result(unsafe { libc::fstat(fd.as_raw_fd(), &mut stat) })?;
    Ok(stat)
}

#[derive(Debug)]
pub(crate) struct Terminal {
    file: TerminalFile,
    same_as_stdin: bool,
    same_as_stdout: bool,
    same_as_stderr: bool,
}

impl Terminal {
    pub(crate) fn lock_stdio(&self) -> StdioLocks {
        StdioLocks {
            stdin_lock: self.same_as_stdin.then(|| stdin().lock()),
            stdout_lock: self.same_as_stdout.then(|| stdout().lock()),
            stderr_lock: self.same_as_stderr.then(|| stderr().lock()),
        }
    }

    pub(crate) fn enable_raw_mode(&mut self) -> io::Result<RawModeGuard<'_>> {
        let fd = self.file.as_fd();
        let old_termios = attr::get_terminal_attr(fd)?;

        if !attr::is_raw_mode_enabled(&old_termios) {
            let mut termios = old_termios;
            attr::enable_raw_mode(&mut termios);
            attr::set_terminal_attr(fd, &termios)?;
            Ok(RawModeGuard {
                inner: self,
                old_termios: Some(old_termios),
            })
        } else {
            Ok(RawModeGuard {
                inner: self,
                old_termios: None,
            })
        }
    }
}

impl Terminal {
    fn from_stdio(file: TerminalFile) -> io::Result<Self> {
        Ok(Terminal {
            same_as_stdin: is_same_file(file.as_fd(), stdin().as_fd())?,
            same_as_stdout: is_same_file(file.as_fd(), stdout().as_fd())?,
            same_as_stderr: is_same_file(file.as_fd(), stderr().as_fd())?,
            file,
        })
    }

    fn from_controlling(file: TerminalFile) -> Self {
        Terminal {
            file,
            same_as_stdin: false,
            same_as_stdout: false,
            same_as_stderr: false,
        }
    }
}

#[derive(Debug)]
enum TerminalFile {
    Owned(File),
    Borrowed(ManuallyDrop<File>),
}

impl io::Write for Terminal {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.file.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.file.flush()
    }
}

impl io::Read for Terminal {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.file.read(buf)
    }
}

impl Deref for TerminalFile {
    type Target = File;

    fn deref(&self) -> &Self::Target {
        match self {
            TerminalFile::Owned(f) => f,
            TerminalFile::Borrowed(f) => f,
        }
    }
}

impl DerefMut for TerminalFile {
    fn deref_mut(&mut self) -> &mut Self::Target {
        match self {
            TerminalFile::Owned(f) => f,
            TerminalFile::Borrowed(f) => f,
        }
    }
}

impl AsFd for super::Terminal {
    fn as_fd(&self) -> std::os::unix::prelude::BorrowedFd<'_> {
        self.0.file.as_fd()
    }
}

impl AsFd for super::TerminalLock<'_> {
    fn as_fd(&self) -> std::os::unix::prelude::BorrowedFd<'_> {
        self.inner.file.as_fd()
    }
}

impl AsFd for super::RawModeGuard<'_> {
    fn as_fd(&self) -> std::os::unix::prelude::BorrowedFd<'_> {
        self.0.inner.file.as_fd()
    }
}

impl AsRawFd for super::Terminal {
    fn as_raw_fd(&self) -> std::os::unix::prelude::RawFd {
        self.0.file.as_raw_fd()
    }
}

impl AsRawFd for super::TerminalLock<'_> {
    fn as_raw_fd(&self) -> std::os::unix::prelude::RawFd {
        self.inner.file.as_raw_fd()
    }
}

impl AsRawFd for super::RawModeGuard<'_> {
    fn as_raw_fd(&self) -> std::os::unix::prelude::RawFd {
        self.0.inner.file.as_raw_fd()
    }
}

pub(crate) struct RawModeGuard<'a> {
    pub(crate) inner: &'a mut Terminal,
    old_termios: Option<termios>,
}

impl fmt::Debug for RawModeGuard<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RawModeGuard")
            .field("inner", &self.inner)
            .finish_non_exhaustive()
    }
}

impl Drop for RawModeGuard<'_> {
    fn drop(&mut self) {
        if let Some(old_termios) = self.old_termios {
            _ = attr::set_terminal_attr(self.inner.file.as_fd(), &old_termios);
        }
    }
}

fn to_io_result(value: c_int) -> io::Result<c_int> {
    if value == -1 {
        Err(io::Error::last_os_error())
    } else {
        Ok(value)
    }
}

// TODO: grow buffer if too small
#[cfg(not(target_os = "macos"))]
fn ptsname_r(fd: BorrowedFd) -> io::Result<CString> {
    let mut buf = vec![0; 256];
    let code = unsafe { libc::ptsname_r(fd.as_raw_fd(), buf.as_mut_ptr().cast(), buf.len()) };
    if code == 0 {
        Ok(unsafe { CStr::from_ptr(buf.as_ptr()).to_owned() })
    } else {
        Err(io::Error::from_raw_os_error(code))
    }
}

#[cfg(target_os = "macos")]
fn ptsname_r(fd: BorrowedFd) -> io::Result<CString> {
    // This is based on
    // https://github.com/Mobivity/nix-ptsname_r-shim/blob/master/src/lib.rs
    // which in turn is based on
    // https://blog.tarq.io/ptsname-on-osx-with-rust/
    // and its derivative
    // https://github.com/philippkeller/rexpect/blob/a71dd02/src/process.rs#L67
    use libc::{c_ulong, ioctl, TIOCPTYGNAME};

    // the buffer size on OSX is 128, defined by sys/ttycom.h
    let buf: [i8; 128] = [0; 128];

    unsafe {
        match ioctl(fd.as_raw_fd(), TIOCPTYGNAME as c_ulong, &buf) {
            0 => {
                let res = CStr::from_ptr(buf.as_ptr()).to_owned();
                Ok(res)
            }
            _ => Err(io::Error::last_os_error()),
        }
    }
}
