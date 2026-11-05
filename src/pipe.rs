//! Bounded FIFO pipe with explicit end-of-stream and backpressure.
use std::collections::VecDeque;

const DEFAULT_CAPACITY: usize = 4096;

pub struct Pipe {
    buf: VecDeque<u8>,
    cap: usize,
    closed: bool,
}
impl Pipe {
    pub fn new(cap: usize) -> Self {
        Self {
            buf: VecDeque::new(),
            cap,
            closed: false,
        }
    }
    /// Returns the accepted prefix length; closed/full pipes accept no bytes.
    pub fn write(&mut self, data: &[u8]) -> usize {
        if self.closed {
            return 0;
        }
        let n = self.remaining_capacity().min(data.len());
        self.buf.extend(&data[..n]);
        n
    }
    pub fn read(&mut self, n: usize) -> Vec<u8> {
        let n = n.min(self.buf.len());
        self.buf.drain(..n).collect()
    }
    pub fn close(&mut self) {
        self.closed = true;
    }
    pub fn is_eof(&self) -> bool {
        self.closed && self.buf.is_empty()
    }
    pub fn available(&self) -> usize {
        self.buf.len()
    }
    pub fn remaining_capacity(&self) -> usize {
        if self.closed {
            0
        } else {
            self.cap.saturating_sub(self.buf.len())
        }
    }
}

impl Default for Pipe {
    fn default() -> Self {
        Self::new(DEFAULT_CAPACITY)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn drains_after_close() {
        let mut p = Pipe::new(3);
        assert_eq!(p.write(b"abcd"), 3);
        assert_eq!(p.read(1), b"a");
        assert_eq!(p.write(b"de"), 1);
        p.close();
        assert_eq!(p.write(b"x"), 0);
        assert!(!p.is_eof());
        assert_eq!(p.read(99), b"bcd");
        assert!(p.is_eof());
    }

    #[test]
    fn zero_capacity_never_accepts_data_and_is_not_eof_until_closed() {
        let mut pipe = Pipe::new(0);
        assert_eq!(pipe.write(b"x"), 0);
        assert!(pipe.read(1).is_empty());
        assert_eq!(pipe.remaining_capacity(), 0);
        assert!(!pipe.is_eof());
        pipe.close();
        assert!(pipe.is_eof());
    }

    #[test]
    fn open_empty_pipe_is_not_eof_and_reads_are_nonblocking() {
        let mut pipe = Pipe::new(4);
        assert!(pipe.read(1).is_empty());
        assert!(!pipe.is_eof());
        assert_eq!(pipe.write(b"data"), 4);
        assert_eq!(pipe.read(2), b"da");
        assert_eq!(pipe.remaining_capacity(), 2);
    }

    #[test]
    fn default_pipe_has_usable_capacity() {
        let mut pipe = Pipe::default();
        assert_eq!(pipe.remaining_capacity(), DEFAULT_CAPACITY);
        assert_eq!(pipe.write(b"data"), 4);
        assert_eq!(pipe.read(4), b"data");
    }
}
