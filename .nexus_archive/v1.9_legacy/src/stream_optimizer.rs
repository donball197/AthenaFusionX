/**
 * Copyright 2026 Donald Ball
 * Project Nexus Singularity v0.2.2
 */
use bytes::{Bytes, BytesMut};
use futures_util::Stream;
use std::pin::Pin;
use std::task::{Context, Poll};

pub struct NeuralChunker<S> {
    stream: S,
    buffer: BytesMut,
    chunk_size: usize,
}

impl<S> NeuralChunker<S> {
    pub fn new(stream: S, chunk_size: usize) -> Self {
        Self { stream, buffer: BytesMut::with_capacity(chunk_size * 2), chunk_size }
    }
}

impl<S> Stream for NeuralChunker<S>
where
    S: Stream<Item = Result<Bytes, reqwest::Error>> + Unpin,
{
    type Item = Result<String, String>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        // Continue polling the inner stream until we reach chunk_size or EOF
        while self.buffer.len() < self.chunk_size {
            match Pin::new(&mut self.stream).poll_next(cx) {
                Poll::Ready(Some(Ok(bytes))) => self.buffer.extend_from_slice(&bytes),
                Poll::Ready(Some(Err(e))) => return Poll::Ready(Some(Err(e.to_string()))),
                Poll::Ready(None) => break,
                Poll::Pending => return Poll::Pending,
            }
        }

        if self.buffer.is_empty() {
            return Poll::Ready(None);
        }

        // Split the buffer at the chunk boundary or remaining length
        let split_at = std::cmp::min(self.buffer.len(), self.chunk_size);
        let chunk = self.buffer.split_to(split_at);

        Poll::Ready(Some(Ok(String::from_utf8_lossy(&chunk).to_string())))
    }
}
