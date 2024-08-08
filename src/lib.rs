//! # IO Pipe library
//! This library add a thread safe way to create multi writers and single reader pipeline.
//! Best way to use that library is writing bytes in few threads and reading that bytes in another single thread.
//!
//! Single thread usage example:
//! ```rust
//! use std::io::{read_to_string, Write};
//!
//! let (mut writer, reader) = io_pipe::pipe();
//! _ = writer.write("hello".as_bytes()).unwrap();
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
//!         _ = writer.write("hello".as_bytes()).unwrap();
//!     }
//! });
//!
//! assert_eq!("hello".len(), read_to_string(reader).unwrap().len());
//! ```

use std::io::IoSlice;
use std::{
    io::{Error, ErrorKind, Read, Result as IOResult, Write},
    sync::mpsc::{channel, Receiver, Sender},
};

type Data = Vec<u8>;

/// ## Create multi writer and single reader objects
///
/// Example
/// ```rust
/// use std::io::{read_to_string, Write};
///
/// let (mut writer, reader) = io_pipe::pipe();
/// _ = writer.write("hello".as_bytes()).unwrap();
/// drop(writer);
///
/// assert_eq!("hello".to_string(), read_to_string(reader).unwrap());
/// ```
pub fn pipe() -> (Writer, Reader) {
    let (sender, receiver) = channel();

    (
        Writer { sender },
        Reader {
            receiver,
            buf: Data::new(),
        },
    )
}

/// ## Multi writer
/// You can clone this writer to write bytes from different threads.
/// Example
/// ```rust
/// use std::io::Write;
///
/// let (mut writer, reader) = io_pipe::pipe();
/// _ = writer.write("hello".as_bytes()).unwrap();
/// ```
///
/// Write method will return an error, when reader dropped.
/// ```rust
/// use std::io::Write;
///
/// let (mut writer, reader) = io_pipe::pipe();
/// drop(reader);
/// assert!(writer.write("hello".as_bytes()).is_err());
/// ```
#[derive(Clone, Debug)]
pub struct Writer {
    sender: Sender<Data>,
}

/// ## Single reader
/// The reader will produce bytes until all writers not dropped.
///
/// Example:
/// ```rust
/// use std::io::{read_to_string, Write};
/// use io_pipe::pipe;
/// let (mut writer, reader) = pipe();
/// _ = writer.write("hello".as_bytes()).unwrap();
/// drop(writer);
///
/// assert_eq!("hello".to_string(), read_to_string(reader).unwrap());
/// ```
///
/// Important: easies case to get deadlock is read from reader when writer is not dropped in single thread
#[derive(Debug)]
pub struct Reader {
    receiver: Receiver<Data>,
    buf: Data,
}

impl Write for Writer {
    fn write(&mut self, buf: &[u8]) -> IOResult<usize> {
        match self.sender.send(buf.to_vec()) {
            Ok(_) => Ok(buf.len()),
            Err(e) => Err(Error::new(ErrorKind::WriteZero, e)),
        }
    }

    fn write_vectored(&mut self, bufs: &[IoSlice<'_>]) -> IOResult<usize> {
        let data_len = bufs.iter().map(|b| b.len()).sum();
        let mut data = Data::with_capacity(data_len);

        for buf in bufs {
            data.extend_from_slice(buf)
        }

        match self.sender.send(data) {
            Ok(_) => Ok(data_len),
            Err(e) => Err(Error::new(ErrorKind::WriteZero, e)),
        }
    }

    fn flush(&mut self) -> IOResult<()> {
        Ok(())
    }
}

impl Read for Reader {
    fn read(&mut self, mut buf: &mut [u8]) -> IOResult<usize> {
        if let Ok(data) = self.receiver.recv() {
            self.buf.extend(data);
        }

        let n = buf.write(self.buf.as_ref())?;
        self.buf.drain(..n);
        Ok(n)
    }
}
#[cfg(test)]
mod tests {
    use std::io::{read_to_string, IoSlice, Write};
    use std::thread::spawn;

    use crate::pipe;

    #[test]
    fn base_case() {
        let (mut writer, reader) = pipe();
        _ = writer.write("hello ".as_bytes()).unwrap();
        _ = writer.write("world".as_bytes()).unwrap();
        drop(writer);

        assert_eq!("hello world".to_string(), read_to_string(reader).unwrap());
    }
    #[test]
    fn base_vectored_case() {
        let (mut writer, reader) = pipe();
        _ = writer
            .write_vectored(&[
                IoSlice::new("hello ".as_bytes()),
                IoSlice::new("world".as_bytes()),
            ])
            .unwrap();
        drop(writer);

        assert_eq!("hello world".to_string(), read_to_string(reader).unwrap());
    }

    #[test]
    fn thread_case() {
        let (writer, reader) = pipe();
        spawn({
            let mut writer = writer.clone();
            move || {
                _ = writer.write("hello".as_bytes()).unwrap();
            }
        });
        spawn({
            let mut writer = writer;
            move || {
                _ = writer.write("world".as_bytes()).unwrap();
            }
        });

        assert_eq!("helloworld".len(), read_to_string(reader).unwrap().len());
    }

    #[test]
    fn writer_err_case() {
        let (mut writer, reader) = pipe();
        drop(reader);

        assert!(writer.write("hello".as_bytes()).is_err());
    }
}
