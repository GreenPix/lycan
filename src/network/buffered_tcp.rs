use std::io::Error as IoError;
use std::io::{Read,Write,ErrorKind};
use std::ops::Deref;

use tokio_core::io::Io;
use bytes::{Buf,MutBuf,RingBuf};
use futures;
use futures::Future;
use futures::Async;
use futures::Poll;
use futures::task::TaskRc;

pub struct BufferedReader<T> {
    inner: T,
    buffer: RingBuf,
}

impl <T: Io> BufferedReader<T> {
    pub fn new(inner: T) -> BufferedReader<T> {
        BufferedReader::with_capacity(inner, 8 * 1024)
    }

    pub fn with_capacity(inner: T, capacity: usize) -> BufferedReader<T> {
        BufferedReader {
            inner: inner,
            buffer: RingBuf::new(capacity),
        }
    }

    pub fn available(&self) -> usize {
        Buf::remaining(&self.buffer)
    }

    pub fn capacity(&self) -> usize {
        self.buffer.capacity()
    }

    /// Returns a future that will complete when at least n bytes are available to read
    ///
    /// # Panics
    /// Panics if n > capacity
    pub fn ensure(self, n: usize) -> Ensure<T> {
        assert!(n <= self.buffer.capacity());
        Ensure {
            buf_read: Some(self),
            objective: n,
        }
    }

    // Try to read at least n bytes from the socket
    fn read_from_socket(&mut self, n: usize) -> Result<bool,IoError> {
        let mut read = 0;
        loop {
            if read >= n {
                return Ok(true);
            }
            let nb = {
                // XXX: Unsafety notice (not entirely solved ...)
                // The two unsafe calls bellow are because it exposes potentially uninitialized memory
                // Everything should work correctly if the type T respects the std::io::Read contract
                // However, it is possible that a user of the library implements a "malicious" one
                // in safe code only, and can thus get access to uninitialized memory
                let to_read = unsafe { MutBuf::mut_bytes(&mut self.buffer) };
                match self.inner.read(to_read) {
                    Ok(nb) => nb,
                    Err(e) => {
                        if e.kind() == ErrorKind::WouldBlock {
                            if read == 0 {
                                return Err(e);
                            } else {
                                return Ok(false);
                            }
                        } else {
                            return Err(e);
                        }
                    }
                }
            };
            // When a peer disconnects, a read to the socket will return Ok(0)
            if nb == 0 {
                return Ok(false);
            }
            unsafe { MutBuf::advance(&mut self.buffer, nb) };
            read += nb;
        }

    }
}

impl <T: Io> Read for BufferedReader<T> {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize,IoError> {
        let mut written = 0;
        loop {
            let b = &mut buf[written..];
            let r = Buf::read_slice(&mut self.buffer, b);
            written += r;
            if b.len() == r {
                // We already filled the buffer
                return Ok(written);
            }

            debug_assert!(!Buf::has_remaining(&self.buffer));

            match self.read_from_socket(b.len()) {
                Ok(finished) => {
                    if !finished && !Buf::has_remaining(&self.buffer) {
                        // We read 0 from the socket (possible disconnect)
                        // Get out of the loop
                        return Ok(written);
                    }
                }
                Err(e) => {
                    if e.kind() == ErrorKind::WouldBlock {
                        if written == 0 {
                            return Err(e);
                        } else {
                            return Ok(written);
                        }
                    } else {
                        return Err(e);
                    }
                }
            }
        }
    }
}

impl <T: Io> Write for BufferedReader<T> {
    fn write(&mut self, buf: &[u8]) -> Result<usize, IoError> {
        self.inner.write(buf)
    }

    fn flush(&mut self) -> Result<(), IoError> {
        self.inner.flush()
    }
}

impl <T: Io> Io for BufferedReader<T> {
    fn poll_read(&mut self) -> Async<()> {
        if !self.buffer.is_empty() {
            Async::Ready(())
        } else {
            self.inner.poll_read()
        }
    }

    fn poll_write(&mut self) -> Async<()> { self.inner.poll_write() }
}

pub struct Ensure<T> {
    buf_read: Option<BufferedReader<T>>,
    objective: usize,
}

impl <T: Io> Future for Ensure<T> {
    type Item = BufferedReader<T>;
    type Error = IoError;

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        match self.buf_read.take() {
            Some(mut b) => {
                let available = b.available();
                if available >= self.objective {
                    Ok(Async::Ready(b))
                } else {
                    match b.read_from_socket(self.objective - available) {
                        Ok(true) => {
                            Ok(Async::Ready(b))
                        }
                        Ok(false) => {
                            self.buf_read = Some(b);
                            Ok(Async::NotReady)
                        }
                        Err(e) => {
                            if e.kind() == ErrorKind::WouldBlock {
                                self.buf_read = Some(b);
                                Ok(Async::NotReady)
                            } else {
                                return Err(e);
                            }
                        }
                    }
                }
            }
            None => panic!("Polling a Ensure after it has resolved"),
        }
    }
}

pub struct IoRef<T> {
    inner: TaskRc<T>,
}

impl <T> Clone for IoRef<T> {
    fn clone(&self) -> IoRef<T> {
        IoRef { inner: self.inner.clone() }
    }
}

impl <T> Read for IoRef<T>
where for <'a> &'a T: Read {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, IoError> {
        self.inner.with(|mut s| s.read(buf))
    }
}

impl <T> Write for IoRef<T>
where for <'a> &'a T: Write {
    fn write(&mut self, buf: &[u8]) -> Result<usize, IoError> {
        self.inner.with(|mut s| s.write(buf))
    }

    fn flush(&mut self) -> Result<(), IoError> {
        self.inner.with(|mut s| s.flush())
    }
}

impl <T> Io for IoRef<T>
where for <'a> &'a T: Io {
    fn poll_read(&mut self) -> Async<()> {
        self.inner.with(|mut s| s.poll_read())
    }

    fn poll_write(&mut self) -> Async<()> {
        self.inner.with(|mut s| s.poll_write())
    }
}

impl <T> IoRef<T> {
    pub fn new<E>(inner: T) -> impl Future<Item=IoRef<T>,Error=E> {
        futures::lazy(|| Ok(IoRef { inner: TaskRc::new(inner) }))
    }
}

impl <T> Deref for IoRef<T> {
    type Target = TaskRc<T>;

    fn deref(&self) -> &TaskRc<T> {
        &self.inner
    }
}
