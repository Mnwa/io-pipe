//! # IO Pipe library
//! Dependency free and thread safe way to create multi writers and single reader pipeline.
//! Best way to use that library is writing bytes in few threads and reading that bytes in another single thread.
//!
//! Single thread usage example:
//! ```rust
//! use std::io::{read_to_string, Write};
//!
//! let (mut writer, reader) = io_pipe::pipe();
//! writer.write_all("hello".as_bytes()).unwrap();
//! drop(writer);
//!
//! assert_eq!("hello".to_string(), read_to_string(reader).unwrap());
//! ```
//!
//! Multi thread usage example:
//! ```rust
//! use std::io::{read_to_string, Write};
//! use std::thread::spawn;
//! use io_pipe::pipe;
//!
//! let (mut writer, reader) = pipe();
//! spawn({
//!     move || {
//!         writer.write_all("hello".as_bytes()).unwrap();
//!     }
//! });
//!
//! assert_eq!("hello".len(), read_to_string(reader).unwrap().len());
//! ```

#[cfg(feature = "async")]
pub use async_pipe::{async_pipe, AsyncReader, AsyncWriter};
#[cfg(feature = "sync")]
pub use sync_pipe::{pipe, Reader, Writer};

#[cfg(feature = "async")]
mod async_pipe;
#[cfg(feature = "sync")]
mod sync_pipe;
