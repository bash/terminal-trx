use std::sync::Arc;
use std::{error, fmt, io};

/// Workaround because `io::Error` is not cloneable.
/// Raw OS errors are truly cloned. Other errors are wrapped in a new `io::Error`.
pub(crate) fn shared_io_error(e: &Arc<io::Error>) -> io::Error {
    e.raw_os_error()
        .map(io::Error::from_raw_os_error)
        .unwrap_or_else(|| io::Error::new(e.kind(), SharedIoError(e.clone())))
}

struct SharedIoError(Arc<io::Error>);

impl fmt::Display for SharedIoError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl fmt::Debug for SharedIoError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl error::Error for SharedIoError {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        self.0.source()
    }
}
