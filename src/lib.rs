//! # IO Pipe Library
//!
//! A thread-safe library for creating multi-writer and single-reader pipelines.
//! This library is ideal for scenarios where you need to write bytes from multiple threads and read them from a single thread.
//!
//! ## Features
//!
//! - Thread-safe communication between writers and readers
//! - Support for both synchronous and asynchronous operations (via feature flags)
//! - Easy-to-use API for creating pipes
//!
//! ## Usage
//!
//! ### Synchronous API
//!
//! Enabled by default. To disable it, disable default features in your `Cargo.toml`:
//!
//! ```toml
//! [dependencies]
//! io_pipe = { version = "*", default-features = false }
//! ```
//!
//! #### Single-thread Example
//!
//! ```rust
//! use std::io::{read_to_string, Write};
//! use io_pipe::pipe;
//!
//! let (mut writer, reader) = pipe();
//! writer.write_all("hello".as_bytes()).unwrap();
//! drop(writer);
//!
//! assert_eq!("hello".to_string(), read_to_string(reader).unwrap());
//! ```
//!
//! #### Multi-thread Example
//!
//! ```rust
//! use std::io::{read_to_string, Write};
//! use std::thread::spawn;
//! use io_pipe::pipe;
//!
//! let (mut writer, reader) = pipe();
//! spawn(move || {
//!     writer.write_all("hello".as_bytes()).unwrap();
//! });
//!
//! assert_eq!("hello".len(), read_to_string(reader).unwrap().len());
//! ```
//!
//! ### Asynchronous API
//!
//! Enable the `async` feature in your `Cargo.toml`:
//!
//! ```toml
//! [dependencies]
//! io_pipe = { version = "*", features = ["async"] }
//! ```
//!
//! #### Async Example
//!
//! ```rust
//! use io_pipe::async_pipe;
//! use futures::io::AsyncWriteExt;
//! use futures::io::AsyncReadExt;
//! use futures::executor::block_on;
//!
//! block_on(async {
//!     let (mut writer, mut reader) = async_pipe();
//!     
//!     writer.write_all(b"hello").await.unwrap();
//!     drop(writer);
//!
//!     let mut buffer = String::new();
//!     reader.read_to_string(&mut buffer).await.unwrap();
//!     assert_eq!("hello", buffer);
//! })
//! ```
//!
//! ## API Documentation
//!
//! For detailed API documentation, please refer to the individual module documentation:
//!
//! - [`pipe`], [`Writer`], and [`Reader`] for synchronous operations
//! - [`async_pipe`], [`AsyncWriter`], and [`AsyncReader`] for asynchronous operations

#[cfg(feature = "async")]
pub use async_pipe::{async_pipe, AsyncReader, AsyncWriter};
#[cfg(feature = "async")]
#[cfg(feature = "sync")]
pub use async_pipe::{async_reader_pipe, async_writer_pipe};
#[cfg(feature = "sync")]
pub use sync_pipe::{pipe, Reader, Writer};

#[cfg(feature = "async")]
mod async_pipe;
mod state;
#[cfg(feature = "sync")]
mod sync_pipe;
