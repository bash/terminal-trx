use std::io::{BufRead, BufReader, Write as _};
use terminal_trx::terminal;

fn main() {
    let mut t = terminal().unwrap();
    let mut l = t.lock().unwrap();
    let mut raw = l.enable_raw_mode().unwrap();
    write!(raw, "\x1b[c").unwrap();
    let mut reader = BufReader::new(&mut raw);
    let mut s = Vec::new();
    reader.read_until(b'\x1b', &mut s).unwrap();
    reader.read_until(b'c', &mut s).unwrap();
    dbg!(String::from_utf8(s).unwrap());
}
