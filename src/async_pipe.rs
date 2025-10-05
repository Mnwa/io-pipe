use std::collections::VecDeque;
use std::future::Future;
use std::io::{BufRead, Error, ErrorKind, IoSlice, Write};
use std::pin::Pin;
use std::task::{Context, Poll};

use futures_io::{AsyncBufRead, AsyncRead, AsyncWrite};
use loole::{Receiver, RecvFuture, SendFuture, Sender, unbounded};

use crate::state::SharedState;

/// Creates a pair of asynchronous writer and reader objects.
///
/// This function returns a tuple containing an `AsyncWriter` and an `AsyncReader`.
/// The `AsyncWriter` can be used to write data, which can then be read from the `AsyncReader`.
///
/// # Arguments
///
/// * `buffer_size` - The size of the internal buffer used for communication between the writer and reader.
///
/// # Returns
///
/// A tuple containing `(AsyncWriter, AsyncReader)`.
///
/// # Example
///
/// ```rust
/// use io_pipe::async_pipe;
///
/// let (writer, reader) = async_pipe();
/// // Use writer to write data and reader to read data asynchronously
/// ```
pub fn async_pipe() -> (AsyncWriter, AsyncReader) {
    let (sender, receiver) = unbounded();
    let state = SharedState::default();

    (
        AsyncWriter {
            sender,
            state: state.clone(),
            write_state: None,
        },
        AsyncReader {
            receiver,
            state,
            buf: VecDeque::new(),
            reading: None,
        },
    )
}

/// Creates a pair of synchronous writer and asynchronous reader objects.
///
/// This function returns a tuple containing an `Writer` and an `AsyncReader`.
/// The `Writer` can be used to write data, which can then be read from the `AsyncReader`.
///
/// # Arguments
///
/// * `buffer_size` - The size of the internal buffer used for communication between the writer and reader.
///
/// # Returns
///
/// A tuple containing `(Writer, AsyncReader)`.
///
/// # Example
///
/// ```rust
/// use io_pipe::async_reader_pipe;
///
/// let (writer, reader) = async_reader_pipe();
/// // Use writer to write data synchronously and reader to read data asynchronously
/// ```
#[cfg(feature = "async")]
#[cfg(feature = "sync")]
pub fn async_reader_pipe() -> (crate::Writer, AsyncReader) {
    let (sender, receiver) = unbounded();
    let state = SharedState::default();

    (
        crate::Writer {
            sender,
            state: state.clone(),
        },
        AsyncReader {
            receiver,
            state,
            buf: VecDeque::new(),
            reading: None,
        },
    )
}

/// Creates a pair of synchronous writer and asynchronous reader objects.
///
/// This function returns a tuple containing an `Writer` and an `AsyncReader`.
/// The `Writer` can be used to write data, which can then be read from the `AsyncReader`.
///
/// # Arguments
///
/// * `buffer_size` - The size of the internal buffer used for communication between the writer and reader.
///
/// # Returns
///
/// A tuple containing `(Writer, AsyncReader)`.
///
/// # Example
///
/// ```rust
/// use io_pipe::async_writer_pipe;
///
/// let (writer, reader) = async_writer_pipe();
/// // Use writer to write data synchronously and reader to read data asynchronously
/// ```
#[cfg(feature = "async")]
#[cfg(feature = "sync")]
pub fn async_writer_pipe() -> (AsyncWriter, crate::Reader) {
    let (sender, receiver) = unbounded();
    let state = SharedState::default();

    (
        AsyncWriter {
            sender,
            state: state.clone(),
            write_state: None,
        },
        crate::Reader {
            receiver,
            state,
            buf: VecDeque::new(),
        },
    )
}

#[derive(Debug)]
struct WriteState {
    send_future: SendFuture<()>,
    n: usize,
}

/// An asynchronous writer that implements `AsyncWrite`.
///
/// This struct allows writing data asynchronously, which can be read from a corresponding `AsyncReader`.
#[derive(Debug)]
pub struct AsyncWriter {
    sender: Sender<()>,
    write_state: Option<WriteState>,
    state: SharedState,
}

impl Clone for AsyncWriter {
    fn clone(&self) -> Self {
        Self {
            sender: self.sender.clone(),
            write_state: None,
            state: self.state.clone(),
        }
    }
}

impl AsyncWriter {
    fn poll_send(&mut self, cx: &mut Context<'_>) -> Poll<std::io::Result<usize>> {
        debug_assert!(self.write_state.is_some());
        match Pin::new(&mut self.write_state.as_mut().unwrap().send_future).poll(cx) {
            Poll::Ready(Ok(_)) => {
                let n = self.write_state.as_ref().map(|s| s.n).unwrap();
                self.write_state = None;
                Poll::Ready(Ok(n))
            }
            Poll::Ready(Err(e)) => {
                self.write_state = None;
                Poll::Ready(Err(Error::new(ErrorKind::WriteZero, e)))
            }
            Poll::Pending => Poll::Pending,
        }
    }
}

impl AsyncWrite for AsyncWriter {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<std::io::Result<usize>> {
        if self.write_state.is_none() {
            self.write_state = Some(WriteState {
                send_future: self.sender.send_async(()),
                n: self.state.write(buf)?,
            });
        }
        match self.poll_send(cx)? {
            Poll::Ready(n) => Poll::Ready(Ok(n)),
            Poll::Pending => Poll::Pending,
        }
    }

    fn poll_write_vectored(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        bufs: &[IoSlice<'_>],
    ) -> Poll<std::io::Result<usize>> {
        if self.write_state.is_none() {
            self.write_state = Some(WriteState {
                send_future: self.sender.send_async(()),
                n: self.state.write_vectored(bufs)?,
            });
        }
        match self.poll_send(cx)? {
            Poll::Ready(n) => Poll::Ready(Ok(n)),
            Poll::Pending => Poll::Pending,
        }
    }

    fn poll_flush(mut self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        Poll::Ready(self.state.flush())
    }

    fn poll_close(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        self.sender.close();
        Poll::Ready(Ok(()))
    }
}

/// An asynchronous reader that implements `AsyncRead` and `AsyncBufRead`.
///
/// This struct allows reading data asynchronously that was written to a corresponding `AsyncWriter`.
#[derive(Debug)]
pub struct AsyncReader {
    receiver: Receiver<()>,
    buf: VecDeque<u8>,
    reading: Option<RecvFuture<()>>,
    state: SharedState,
}

impl AsyncBufRead for AsyncReader {
    fn poll_fill_buf(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<&[u8]>> {
        let this = self.get_mut();
        while this.buf.is_empty() {
            let n = this.state.copy_to(&mut this.buf)?;
            if n == 0 {
                if this.reading.is_none() {
                    this.reading = Some(this.receiver.recv_async())
                }

                match Pin::new(this.reading.as_mut().unwrap()).poll(cx) {
                    Poll::Ready(Ok(_)) => {
                        this.reading = None;
                    }
                    Poll::Ready(Err(_)) => {
                        this.reading = None;
                        break;
                    }
                    Poll::Pending => return Poll::Pending,
                }
            }
        }

        if this.buf.is_empty() {
            _ = this.state.copy_to(&mut this.buf)?;
        }

        Poll::Ready(this.buf.fill_buf())
    }

    fn consume(mut self: Pin<&mut Self>, amt: usize) {
        self.buf.consume(amt)
    }
}

impl AsyncRead for AsyncReader {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        mut buf: &mut [u8],
    ) -> Poll<std::io::Result<usize>> {
        let data = match self.as_mut().poll_fill_buf(cx)? {
            Poll::Ready(buf) => buf,
            Poll::Pending => return Poll::Pending,
        };
        let n = buf.write(data)?;
        self.consume(n);
        Poll::Ready(Ok(n))
    }
}

#[cfg(test)]
mod tests {
    use std::io::IoSlice;
    use std::thread::spawn;

    use futures::{
        AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, StreamExt, TryStreamExt, executor::block_on,
    };

    #[test]
    fn base_write_case() {
        block_on(async {
            // Checking non-blocking buffer inside writer
            let (mut writer, reader) = crate::async_pipe();
            for _ in 0..1000 {
                writer.write_all("hello".as_bytes()).await.unwrap();
            }
            drop(reader)
        })
    }

    #[test]
    fn base_read_case() {
        block_on(async {
            let (mut writer, mut reader) = crate::async_pipe();

            writer.write_all("hello ".as_bytes()).await.unwrap();
            writer.write_all("world".as_bytes()).await.unwrap();
            drop(writer);

            let mut str = String::new();
            reader.read_to_string(&mut str).await.unwrap();

            assert_eq!("hello world".to_string(), str);
        });
    }

    #[test]
    fn base_vectored_case() {
        block_on(async {
            let (mut writer, mut reader) = crate::async_pipe();
            _ = writer
                .write_vectored(&[
                    IoSlice::new("hello ".as_bytes()),
                    IoSlice::new("world".as_bytes()),
                ])
                .await
                .unwrap();
            drop(writer);

            let mut str = String::new();
            reader.read_to_string(&mut str).await.unwrap();

            assert_eq!("hello world".to_string(), str);
        });
    }

    #[test]
    fn thread_case() {
        block_on(async {
            let (writer, mut reader) = crate::async_pipe();
            let writers = (0..1000).map(|_| writer.clone()).collect::<Vec<_>>();
            let writers_len = writers.len();
            drop(writer);
            let write_fut = futures::stream::iter(writers)
                .map(|mut writer| async move { writer.write_all("hello".as_bytes()).await })
                .buffer_unordered(writers_len)
                .try_collect::<Vec<()>>();

            let mut str = String::new();
            let read_fut = reader.read_to_string(&mut str);
            futures::join!(
                async {
                    write_fut.await.unwrap();
                },
                async { read_fut.await.unwrap() }
            );

            assert_eq!("hello".repeat(writers_len), str);
        });
    }

    #[test]
    fn writer_err_case() {
        block_on(async {
            let (mut writer, reader) = crate::async_pipe();
            drop(reader);

            assert!(writer.write("hello".as_bytes()).await.is_err());
        });
    }

    #[test]
    fn bufread_case() {
        block_on(async {
            let (mut writer, mut reader) = crate::async_pipe();
            writer.write_all("hello\n".as_bytes()).await.unwrap();
            writer.write_all("world".as_bytes()).await.unwrap();
            drop(writer);

            let mut str = String::new();
            assert_ne!(0, reader.read_line(&mut str).await.unwrap());
            assert_eq!("hello\n".to_string(), str);

            let mut str = String::new();
            assert_ne!(0, reader.read_line(&mut str).await.unwrap());
            assert_eq!("world".to_string(), str);

            let mut str = String::new();
            assert_eq!(0, reader.read_line(&mut str).await.unwrap());
        });
    }

    #[test]
    fn bufread_lines_case() {
        block_on(async {
            let (mut writer, reader) = crate::async_pipe();
            writer.write_all("hello\n".as_bytes()).await.unwrap();
            writer.write_all("world".as_bytes()).await.unwrap();
            drop(writer);

            assert_eq!(2, reader.lines().map(|l| assert!(l.is_ok())).count().await)
        });
    }

    #[test]
    fn thread_writer_case() {
        use std::io::Write;

        let (writer, mut reader) = crate::async_reader_pipe();
        for _ in 0..1000 {
            let mut writer = writer.clone();
            spawn(move || {
                writer.write_all("hello".as_bytes()).unwrap();
            });
        }
        drop(writer);

        block_on(async {
            let mut str = String::new();
            reader.read_to_string(&mut str).await.unwrap();

            assert_eq!("hello".repeat(1000), str);
        })
    }

    #[test]
    fn thread_reader_case() {
        use std::io::Read;

        let (writer, mut reader) = crate::async_writer_pipe();
        for _ in 0..1000 {
            let mut writer = writer.clone();
            spawn(move || {
                block_on(async {
                    writer.write_all("hello".as_bytes()).await.unwrap();
                })
            });
        }
        drop(writer);

        let mut str = String::new();
        reader.read_to_string(&mut str).unwrap();

        assert_eq!("hello".repeat(1000), str);
    }

    #[test]
    fn threads_write_and_read_case() {
        let (writer, mut reader) = crate::async_pipe();

        for _ in 0..1000 {
            let mut writer = writer.clone();

            spawn(move || {
                block_on(async {
                    writer.write_all(&[0; 4]).await.unwrap();
                })
            });

            block_on(async {
                let mut buf = [0; 4];
                assert_eq!(buf.len(), reader.read(&mut buf).await.unwrap());
            })
        }
        drop(writer);
    }
}
