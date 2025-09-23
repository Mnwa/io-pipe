use std::collections::VecDeque;
use std::fmt::Arguments;
use std::io::{BufRead, IoSlice};
use std::io::{Error, ErrorKind, Read, Result as IOResult, Write};

use loole::{Receiver, Sender, unbounded};

use crate::state::SharedState;

/// Creates a pair of synchronous writer and reader objects.
///
/// This function returns a tuple containing a `Writer` and a `Reader`.
/// The `Writer` can be used to write data, which can then be read from the `Reader`.
///
/// # Returns
///
/// A tuple containing `(Writer, Reader)`.
///
/// # Example
///
/// ```rust
/// use std::io::{read_to_string, Write};
/// use io_pipe::pipe;
///
/// let (mut writer, reader) = pipe();
/// writer.write_all("hello".as_bytes()).unwrap();
/// drop(writer);
///
/// assert_eq!("hello".to_string(), read_to_string(reader).unwrap());
/// ```
pub fn pipe() -> (Writer, Reader) {
    let (sender, receiver) = unbounded();

    let state = SharedState::default();

    (
        Writer {
            sender,
            state: state.clone(),
        },
        Reader {
            receiver,
            state,
            buf: VecDeque::new(),
        },
    )
}

/// A synchronous writer that implements `Write`.
///
/// This struct allows writing data synchronously, which can be read from a corresponding `Reader`.
/// Multiple `Writer` instances can be created by cloning, allowing writes from different threads.
///
/// # Notes
///
/// - All write calls are executed immediately without blocking the thread.
/// - It's safe to use this writer inside async operations.
/// - Write method will return an error when the reader is dropped.
///
/// # Example
///
/// ```rust
/// use std::io::Write;
/// use io_pipe::pipe;
///
/// let (mut writer, reader) = pipe();
/// writer.write_all("hello".as_bytes()).unwrap();
/// ```
#[derive(Clone, Debug)]
pub struct Writer {
    pub(crate) sender: Sender<()>,
    pub(crate) state: SharedState,
}

impl Write for Writer {
    fn write(&mut self, buf: &[u8]) -> IOResult<usize> {
        let n = self.state.write(buf)?;
        match self.sender.send(()) {
            Ok(_) => Ok(n),
            Err(e) => Err(Error::new(ErrorKind::WriteZero, e)),
        }
    }

    fn write_vectored(&mut self, bufs: &[IoSlice<'_>]) -> IOResult<usize> {
        let n = self.state.write_vectored(bufs)?;
        match self.sender.send(()) {
            Ok(_) => Ok(n),
            Err(e) => Err(Error::new(ErrorKind::WriteZero, e)),
        }
    }

    fn flush(&mut self) -> IOResult<()> {
        self.state.flush()
    }

    fn write_all(&mut self, buf: &[u8]) -> IOResult<()> {
        self.state.write_all(buf)?;
        match self.sender.send(()) {
            Ok(_) => Ok(()),
            Err(e) => Err(Error::new(ErrorKind::WriteZero, e)),
        }
    }

    fn write_fmt(&mut self, fmt: Arguments<'_>) -> IOResult<()> {
        self.state.write_fmt(fmt)?;
        match self.sender.send(()) {
            Ok(_) => Ok(()),
            Err(e) => Err(Error::new(ErrorKind::WriteZero, e)),
        }
    }
}

/// A synchronous reader that implements `Read` and `BufRead`.
///
/// This struct allows reading data synchronously that was written to a corresponding `Writer`.
/// The reader will produce bytes until all writers are dropped.
///
/// # Notes
///
/// - Reads may block the thread until a writer sends data.
/// - Implements the `BufRead` trait for buffered reading.
/// - Be cautious of potential deadlocks when reading from the reader before dropping the writer in a single thread.
///
/// # Example
///
/// ```rust
/// use std::io::{read_to_string, Write};
/// use io_pipe::pipe;
///
/// let (mut writer, reader) = pipe();
/// writer.write_all("hello".as_bytes()).unwrap();
/// drop(writer);
///
/// assert_eq!("hello".to_string(), read_to_string(reader).unwrap());
/// ```
#[derive(Debug)]
pub struct Reader {
    pub(crate) receiver: Receiver<()>,
    pub(crate) buf: VecDeque<u8>,
    pub(crate) state: SharedState,
}

impl BufRead for Reader {
    fn fill_buf(&mut self) -> IOResult<&[u8]> {
        while self.buf.is_empty() {
            let n = self.state.copy_to(&mut self.buf)?;
            if n == 0 && self.receiver.recv().is_err() {
                break;
            }
        }

        self.buf.fill_buf()
    }

    fn consume(&mut self, amt: usize) {
        self.buf.consume(amt)
    }
}

impl Read for Reader {
    fn read(&mut self, mut buf: &mut [u8]) -> IOResult<usize> {
        let n = buf.write(self.fill_buf()?)?;
        self.consume(n);
        Ok(n)
    }
}
#[cfg(test)]
mod tests {
    use std::io::{BufRead, IoSlice, Read, Write, read_to_string};
    use std::thread::spawn;

    #[test]
    fn base_write_case() {
        // Checking non-blocking buffer inside writer
        let (mut writer, reader) = crate::pipe();
        for _ in 0..1000 {
            writer.write_all("hello".as_bytes()).unwrap();
        }
        drop(reader)
    }

    #[test]
    fn base_read_case() {
        let (mut writer, reader) = crate::pipe();
        writer.write_all("hello ".as_bytes()).unwrap();
        writer.write_all("world".as_bytes()).unwrap();
        drop(writer);

        assert_eq!("hello world".to_string(), read_to_string(reader).unwrap());
    }
    #[test]
    fn base_vectored_case() {
        let (mut writer, reader) = crate::pipe();
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
        let (writer, reader) = crate::pipe();
        for _ in 0..1000 {
            let mut writer = writer.clone();
            spawn(move || {
                writer.write_all("hello".as_bytes()).unwrap();
            });
        }
        drop(writer);

        assert_eq!("hello".repeat(1000), read_to_string(reader).unwrap());
    }

    #[test]
    fn writer_err_case() {
        let (mut writer, reader) = crate::pipe();
        drop(reader);

        assert!(writer.write("hello".as_bytes()).is_err());
    }

    #[test]
    fn bufread_case() {
        let (mut writer, mut reader) = crate::pipe();
        writer.write_all("hello\n".as_bytes()).unwrap();
        writer.write_all("world".as_bytes()).unwrap();
        drop(writer);

        let mut str = String::new();
        assert_ne!(0, reader.read_line(&mut str).unwrap());
        assert_eq!("hello\n".to_string(), str);

        let mut str = String::new();
        assert_ne!(0, reader.read_line(&mut str).unwrap());
        assert_eq!("world".to_string(), str);

        let mut str = String::new();
        assert_eq!(0, reader.read_line(&mut str).unwrap());
    }

    #[test]
    fn bufread_lines_case() {
        let (mut writer, reader) = crate::pipe();
        writer.write_all("hello\n".as_bytes()).unwrap();
        writer.write_all("world".as_bytes()).unwrap();
        drop(writer);

        assert_eq!(2, reader.lines().map(|l| assert!(l.is_ok())).count())
    }

    #[test]
    fn threads_write_and_read_case() {
        let (writer, mut reader) = crate::pipe();

        for _ in 0..1000 {
            let mut writer = writer.clone();
            spawn(move || {
                writer.write_all(&[0; 4]).unwrap();
            });

            let mut buf = [0; 4];
            assert_eq!(buf.len(), reader.read(&mut buf).unwrap());
        }
        drop(writer);
    }
}
