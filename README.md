# IO Pipe library

![Crates.io Version](https://img.shields.io/crates/v/io-pipe)
![GitHub Actions Workflow Status](https://img.shields.io/github/actions/workflow/status/Mnwa/io-pipe/rust.yml)
![docs.rs](https://img.shields.io/docsrs/io-pipe)
![Crates.io License](https://img.shields.io/crates/l/io-pipe)

Dependency free and thread safe way to create multi writers and single reader pipeline.
Best way to use that library is writing bytes in few threads and reading that bytes in another single thread.

## How to

docs.rs documentation is [here](https://docs.rs/io-pipe/latest/io_pipe/)

### Install

```bash
cargo install io-pipe
```

### Use

Single thread usage example:

```rust
use std::io::{read_to_string, Write};

fn main() {
    let (mut writer, reader) = io_pipe::pipe();
    _ = writer.write("hello".as_bytes()).unwrap();
    drop(writer);

    assert_eq!("hello".to_string(), read_to_string(reader).unwrap());
}
```

Multi thread usage example:

```rust
use std::io::{read_to_string, Write};
use std::thread::spawn;
use io_pipe::pipe;

fn main() {
    let (mut writer, reader) = pipe();
    spawn({
        move || {
            _ = writer.write("hello".as_bytes()).unwrap();
        }
    });

    assert_eq!("hello".len(), read_to_string(reader).unwrap().len());
}
```

## Contributing

Feel free for creating new PR.
Use rustfmt and clippy checks before commiting.