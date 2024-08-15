# IO Pipe Library

[![Crates.io Version](https://img.shields.io/crates/v/io-pipe)](https://crates.io/crates/io-pipe)
[![GitHub Actions Workflow Status](https://img.shields.io/github/actions/workflow/status/Mnwa/io-pipe/rust.yml)](https://github.com/Mnwa/io-pipe/actions/workflows/rust.yml?query=branch%3Amaster)
[![docs.rs](https://img.shields.io/docsrs/io-pipe)](https://docs.rs/io-pipe/latest/io_pipe/)
[![Crates.io License](https://img.shields.io/crates/l/io-pipe)](LICENSE)

IO Pipe is a thread-safe Rust library for creating multi-writer and single-reader pipelines. It's
ideal for scenarios where you need to write bytes from multiple threads and read them from a single thread.

## Features

- Thread-safe communication between writers and readers
- Support for both synchronous and asynchronous operations (via feature flags)
- Easy-to-use API for creating pipes
- Zero dependencies (for core functionality)

## Installation

Add this to your `Cargo.toml`:

```toml
[dependencies]
io-pipe = "0.x.x"
```

For async support, enable the `async` feature:

```toml
[dependencies]
io-pipe = { version = "0.x.x", features = ["async"] }
```

## Usage

### Synchronous API

Single-thread example:

```rust
use std::io::{read_to_string, Write};
use io_pipe::pipe;

fn main() {
    let (mut writer, reader) = pipe();
    writer.write_all("hello".as_bytes()).unwrap();
    drop(writer);

    assert_eq!("hello".to_string(), read_to_string(reader).unwrap());
}
```

Multi-thread example:

```rust
use std::io::{read_to_string, Write};
use std::thread::spawn;
use io_pipe::pipe;

fn main() {
    let (mut writer, reader) = pipe();
    spawn(move || {
        writer.write_all("hello".as_bytes()).unwrap();
    });

    assert_eq!("hello".len(), read_to_string(reader).unwrap().len());
}
```

### Asynchronous API

```rust
use io_pipe::async_pipe;
use futures::io::{AsyncWriteExt, AsyncReadExt};
use futures::executor::block_on;

fn main() {
    block_on(async {
        let (mut writer, mut reader) = async_pipe(1024);

        writer.write_all(b"hello").await.unwrap();
        drop(writer);

        let mut buffer = String::new();
        reader.read_to_string(&mut buffer).await.unwrap();
        assert_eq!("hello", buffer);
    });
}
```

### Sync To Asynchronous API

```rust
use io_pipe::async_reader_pipe;
use futures::io::{AsyncWriteExt, AsyncReadExt};
use futures::executor::block_on;

fn main() {
    block_on(async {
        let (mut writer, mut reader) = async_pipe();

        writer.write_all(b"hello").await.unwrap();
        drop(writer);

        let mut buffer = String::new();
        reader.read_to_string(&mut buffer).await.unwrap();
        assert_eq!("hello", buffer);
    });
}
```

#### Sync to Async Example

```rust
use io_pipe::async_reader_pipe;
use std::io::Write;
use futures::io::AsyncReadExt;
use futures::executor::block_on;

fn main() {
    let (mut writer, mut reader) = async_reader_pipe();
    writer.write_all(b"hello").unwrap();
    drop(writer);

    block_on(async {
        let mut buffer = String::new();
        reader.read_to_string(&mut buffer).await.unwrap();
        assert_eq!("hello", buffer);
    })
}
```

#### Async to Sync Example

```rust
use io_pipe::async_writer_pipe;
use std::io::Read;
use futures::io::AsyncWriteExt;
use futures::executor::block_on;

fn main() {
    let (mut writer, mut reader) = async_writer_pipe();

    block_on(async {
        writer.write_all(b"hello").await.unwrap();
        drop(writer);
    });

    let mut buffer = String::new();
    reader.read_to_string(&mut buffer).unwrap();
    assert_eq!("hello", buffer);
}
```

## Documentation

For detailed API documentation, please refer to [docs.rs](https://docs.rs/io-pipe/latest/io_pipe/).

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

Guidelines:

1. Use `rustfmt` to format your code.
2. Run `clippy` and address any lints before submitting your PR.
3. Write tests for new functionality.
4. Update documentation as necessary.

## License

This project is licensed under MIT - see the [LICENSE](LICENSE) file for details.
