# terminal-trx

Provides a handle to the terminal of the current process that is both readable and writable.

## Example
```rust
use terminal_trx::terminal;
use std::io::{BufReader, BufRead as _, Write as _};

let mut terminal = terminal().unwrap();

write!(terminal, "hello world").unwrap();

let mut reader = BufReader::new(&mut terminal);
let mut line = String::new();
reader.read_line(&mut line).unwrap();
```

## License
Licensed under either of

* Apache License, Version 2.0
  ([license-apache.txt](license-apache.txt) or http://www.apache.org/licenses/LICENSE-2.0)
* MIT license
  ([license-mit.txt](license-mit.txt) or http://opensource.org/licenses/MIT)

at your option.

## Contribution
Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions
