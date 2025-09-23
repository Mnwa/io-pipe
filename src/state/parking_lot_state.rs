use parking_lot::Mutex;
use std::collections::VecDeque;
use std::fmt::Arguments;
use std::io::{IoSlice, Read, Write};
use std::sync::Arc;

#[derive(Clone, Debug, Default)]
pub(crate) struct State {
    buf: VecDeque<u8>,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct SharedState(Arc<Mutex<State>>);

impl Read for SharedState {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.0.lock().buf.read(buf)
    }
}

impl Write for SharedState {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0.lock().buf.write(buf)
    }

    fn write_vectored(&mut self, bufs: &[IoSlice<'_>]) -> std::io::Result<usize> {
        let mut guard = self.0.lock();
        let n = bufs.iter().map(|b| b.len()).sum::<usize>();
        guard.buf.reserve(n);
        guard.buf.extend(bufs.iter().flat_map(|b| b.as_ref()));
        Ok(n)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.0.lock().buf.flush()
    }

    fn write_all(&mut self, buf: &[u8]) -> std::io::Result<()> {
        self.0.lock().buf.write_all(buf)
    }

    fn write_fmt(&mut self, fmt: Arguments<'_>) -> std::io::Result<()> {
        self.0.lock().buf.write_fmt(fmt)
    }
}
