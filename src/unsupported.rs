use thiserror::Error;

use crate::StdioLocks;
use std::{io, marker::PhantomData};

pub(crate) fn terminal() -> io::Result<Terminal> {
    Err(io::Error::new(io::ErrorKind::Unsupported, UnsupportedError))
}

#[derive(Debug, Error)]
#[error("this platform is not supported")]
struct UnsupportedError;

#[derive(Debug)]
pub(crate) struct Terminal {}

impl io::Write for Terminal {
    fn write(&mut self, _buf: &[u8]) -> io::Result<usize> {
        todo!()
    }

    fn flush(&mut self) -> io::Result<()> {
        todo!()
    }
}

impl io::Read for Terminal {
    fn read(&mut self, _buf: &mut [u8]) -> io::Result<usize> {
        todo!()
    }
}

impl Terminal {
    pub(crate) fn lock_stdio(&mut self) -> StdioLocks {
        todo!()
    }

    pub(crate) fn enable_raw_mode(&mut self) -> io::Result<RawModeGuard<'_>> {
        todo!()
    }
}

#[derive(Debug)]
pub(crate) struct RawModeGuard<'a>(PhantomData<&'a ()>);

impl io::Write for RawModeGuard<'_> {
    fn write(&mut self, _buf: &[u8]) -> io::Result<usize> {
        todo!()
    }

    fn flush(&mut self) -> io::Result<()> {
        todo!()
    }
}

impl io::Read for RawModeGuard<'_> {
    fn read(&mut self, _buf: &mut [u8]) -> io::Result<usize> {
        todo!()
    }
}
