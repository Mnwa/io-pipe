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

use std::{
    io::{Error, ErrorKind, Read, Result as IOResult, Write},
    sync::mpsc::{channel, Receiver, Sender},
};

use bytes::{Buf, BufMut, Bytes, BytesMut};

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
    sender: Sender<Bytes>,
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
    receiver: Receiver<Bytes>,
    buf: BytesMut,
}

impl Write for Writer {
    fn write(&mut self, buf: &[u8]) -> IOResult<usize> {
        match self.sender.send(Bytes::copy_from_slice(buf)) {
            Ok(_) => Ok(buf.len()),
            Err(e) => Err(Error::new(ErrorKind::BrokenPipe, e)),
        }
    }

    fn flush(&mut self) -> IOResult<()> {
        Ok(())
    }
}

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
            buf: BytesMut::new(),
        },
    )
}

impl Read for Reader {
    fn read(&mut self, mut buf: &mut [u8]) -> IOResult<usize> {
        if let Ok(data) = self.receiver.recv() {
            self.buf.put(data);
        }

        let n = buf.write(self.buf.as_ref())?;
        self.buf.advance(n);
        Ok(n)
    }
}
#[cfg(test)]
mod tests {
    use std::io::{read_to_string, Write};
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
