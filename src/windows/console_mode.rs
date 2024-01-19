use std::io;
use std::os::windows::io::{AsRawHandle as _, BorrowedHandle};

use windows_sys::Win32::System::Console::{
    GetConsoleMode, SetConsoleMode, CONSOLE_MODE, ENABLE_ECHO_INPUT, ENABLE_LINE_INPUT,
};

use super::to_io_result;

// We disable two flags:
// ENABLE_ECHO_INPUT
//     to disable input characters from being echoed.
// ENABLE_LINE_INPUT
//     We want input to be available immediately and not wait for a line terminator
const FLAGS_DISABLED_IN_RAW_MODE: CONSOLE_MODE = ENABLE_ECHO_INPUT | ENABLE_LINE_INPUT;

pub(super) fn get_console_mode(handle: BorrowedHandle) -> io::Result<CONSOLE_MODE> {
    let mut mode = Default::default();
    // SAFETY: Both handle and pointer are valid.
    to_io_result(unsafe { GetConsoleMode(handle.as_raw_handle() as isize, &mut mode) })?;
    Ok(mode)
}

pub(super) fn set_console_mode(handle: BorrowedHandle, mode: CONSOLE_MODE) -> io::Result<()> {
    // SAFETY: Handle is valid (borrowed).
    to_io_result(unsafe { SetConsoleMode(handle.as_raw_handle() as isize, mode) })
}

pub(super) fn is_raw_mode_enabled(mode: CONSOLE_MODE) -> bool {
    mode & FLAGS_DISABLED_IN_RAW_MODE == 0
}

pub(super) fn enable_raw_mode(mode: CONSOLE_MODE) -> CONSOLE_MODE {
    mode & !(FLAGS_DISABLED_IN_RAW_MODE)
}
