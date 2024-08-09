use std::io::{Error, ErrorKind, IoSlice, Write};
use std::pin::Pin;
use std::task::{Context, Poll};

use futures::channel::mpsc::{channel, Receiver, Sender};
use futures::io::{AsyncBufRead, AsyncRead, AsyncWrite};
use futures::{Sink, Stream};

type Data = Vec<u8>;

/// ## Create multi writer and single reader objects
///
/// Example
/// ```rust
/// ```
pub fn async_pipe(buffer_size: usize) -> (AsyncWriter, AsyncReader) {
    let (sender, receiver) = channel(buffer_size);

    (
        AsyncWriter { sender },
        AsyncReader {
            receiver,
            buf: Data::new(),
        },
    )
}

#[derive(Clone, Debug)]
pub struct AsyncWriter {
    sender: Sender<Data>,
}

impl AsyncWrite for AsyncWriter {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<std::io::Result<usize>> {
        if Pin::new(&mut self.sender).poll_ready(cx).is_pending() {
            return Poll::Pending;
        }

        match self.sender.start_send(buf.to_vec()) {
            Ok(_) => Poll::Ready(Ok(buf.len())),
            Err(e) => Poll::Ready(Err(Error::new(ErrorKind::WriteZero, e))),
        }
    }

    fn poll_write_vectored(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        bufs: &[IoSlice<'_>],
    ) -> Poll<std::io::Result<usize>> {
        let data = bufs
            .iter()
            .flat_map(|b| b.as_ref())
            .copied()
            .collect::<Data>();

        let data_len = data.len();

        if Pin::new(&mut self.sender).poll_ready(cx).is_pending() {
            return Poll::Pending;
        }

        match self.sender.start_send(data) {
            Ok(_) => Poll::Ready(Ok(data_len)),
            Err(e) => Poll::Ready(Err(Error::new(ErrorKind::WriteZero, e))),
        }
    }

    fn poll_flush(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        Poll::Ready(Ok(()))
    }

    fn poll_close(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        match Pin::new(&mut self.sender).poll_close(cx) {
            Poll::Ready(Ok(_)) => Poll::Ready(Ok(())),
            Poll::Ready(Err(e)) => Poll::Ready(Err(Error::new(ErrorKind::BrokenPipe, e))),
            Poll::Pending => Poll::Pending,
        }
    }
}

#[derive(Debug)]
pub struct AsyncReader {
    receiver: Receiver<Data>,
    buf: Data,
}

impl AsyncBufRead for AsyncReader {
    fn poll_fill_buf(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<std::io::Result<&[u8]>> {
        if self.buf.is_empty() {
            match Pin::new(&mut self.receiver).poll_next(cx) {
                Poll::Ready(Some(data)) => {
                    self.buf.extend(data);
                }
                Poll::Ready(None) => {}
                Poll::Pending => return Poll::Pending,
            }
        }

        Poll::Ready(Ok(self.get_mut().buf.as_ref()))
    }

    fn consume(mut self: Pin<&mut Self>, amt: usize) {
        self.buf.drain(..amt);
    }
}

impl AsyncRead for AsyncReader {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        mut buf: &mut [u8],
    ) -> Poll<std::io::Result<usize>> {
        let data = match self.as_mut().poll_fill_buf(cx) {
            Poll::Ready(Ok(buf)) => buf,
            Poll::Ready(Err(e)) => return Poll::Ready(Err(e)),
            Poll::Pending => return Poll::Pending,
        };
        let n = match buf.write(data) {
            Ok(n) => n,
            Err(e) => return Poll::Ready(Err(e)),
        };
        self.consume(n);
        Poll::Ready(Ok(n))
    }
}

#[cfg(test)]
mod tests {
    use std::io::IoSlice;

    use futures::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, StreamExt};

    #[test]
    fn base_write_case() {
        futures_executor::block_on(async {
            // Checking non-blocking buffer inside writer
            let (mut writer, reader) = crate::async_pipe(1000);
            for _ in 0..1000 {
                writer.write_all("hello".as_bytes()).await.unwrap();
            }
            drop(reader)
        })
    }

    #[test]
    fn base_read_case() {
        futures_executor::block_on(async {
            let (mut writer, mut reader) = crate::async_pipe(2);

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
        futures_executor::block_on(async {
            let (mut writer, mut reader) = crate::async_pipe(2);
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
        futures_executor::block_on(async {
            let (writer, mut reader) = crate::async_pipe(2);
            futures::join!(
                {
                    let mut writer = writer.clone();
                    async move {
                        writer.write_all("hello".as_bytes()).await.unwrap();
                    }
                },
                {
                    let mut writer = writer;
                    async move {
                        writer.write_all("world".as_bytes()).await.unwrap();
                    }
                }
            );

            let mut str = String::new();
            reader.read_to_string(&mut str).await.unwrap();

            assert_eq!("helloworld".len(), str.len());
        });
    }

    #[test]
    fn writer_err_case() {
        futures_executor::block_on(async {
            let (mut writer, reader) = crate::async_pipe(1);
            drop(reader);

            assert!(writer.write("hello".as_bytes()).await.is_err());
        });
    }

    #[test]
    fn bufread_case() {
        futures_executor::block_on(async {
            let (mut writer, mut reader) = crate::async_pipe(2);
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
        futures_executor::block_on(async {
            let (mut writer, reader) = crate::async_pipe(2);
            writer.write_all("hello\n".as_bytes()).await.unwrap();
            writer.write_all("world".as_bytes()).await.unwrap();
            drop(writer);

            assert_eq!(2, reader.lines().map(|l| assert!(l.is_ok())).count().await)
        });
    }
}
