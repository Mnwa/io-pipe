use std::io::{BufRead, IoSlice};
use std::io::{Error, ErrorKind, Read, Result as IOResult, Write};

use loole::{unbounded, Receiver, Sender};

type Data = Vec<u8>;

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

    (
        Writer { sender },
        Reader {
            receiver,
            buf: Data::new(),
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
    pub(crate) sender: Sender<Data>,
}

impl Write for Writer {
    fn write(&mut self, buf: &[u8]) -> IOResult<usize> {
        match self.sender.send(buf.to_vec()) {
            Ok(_) => Ok(buf.len()),
            Err(e) => Err(Error::new(ErrorKind::WriteZero, e)),
        }
    }

    fn write_vectored(&mut self, bufs: &[IoSlice<'_>]) -> IOResult<usize> {
        let data = bufs
            .iter()
            .flat_map(|b| b.as_ref())
            .copied()
            .collect::<Data>();

        let data_len = data.len();

        match self.sender.send(data) {
            Ok(_) => Ok(data_len),
            Err(e) => Err(Error::new(ErrorKind::WriteZero, e)),
        }
    }

    fn flush(&mut self) -> IOResult<()> {
        Ok(())
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
    pub(crate) receiver: Receiver<Data>,
    pub(crate) buf: Data,
}

impl BufRead for Reader {
    fn fill_buf(&mut self) -> IOResult<&[u8]> {
        if self.buf.is_empty() {
            if let Ok(data) = self.receiver.recv() {
                self.buf.extend(data);
            }
        }
        Ok(self.buf.as_ref())
    }

    fn consume(&mut self, amt: usize) {
        self.buf.drain(..amt);
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
    use std::io::{read_to_string, BufRead, IoSlice, Write};
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

        assert_eq!("hello".len() * 1000, read_to_string(reader).unwrap().len());
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
}
