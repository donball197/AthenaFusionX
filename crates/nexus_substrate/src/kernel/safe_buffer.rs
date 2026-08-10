use bytes::BytesMut;

#[derive(Debug)]
pub enum BufferError {
    Overflow,
}

pub struct SafeBuffer {
    buf: BytesMut,
    max_capacity: usize,
}

impl SafeBuffer {
    pub fn new(max_capacity: usize) -> Self {
        Self { buf: BytesMut::with_capacity(max_capacity), max_capacity }
    }

    pub fn write_safe(&mut self, data: &[u8]) -> Result<(), BufferError> {
        if self.buf.len() + data.len() > self.max_capacity {
            return Err(BufferError::Overflow);
        }
        self.buf.extend_from_slice(data);
        Ok(())
    }
}
