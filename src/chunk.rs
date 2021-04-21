use std::cmp::min;
use std::convert::TryInto;
use std::io::{self, Read, Write, Error};
use parking_lot::{RwLock, RwLockWriteGuard, Mutex};
use tokio::io::{AsyncWrite, AsyncRead, ReadBuf};
use std::task::{Context, Poll, Waker};
use std::pin::Pin;
use std::mem::MaybeUninit;

pub type Block = [u8; BLOCK_SIZE];

const CHUNK_SIZE: usize = BLOCK_SIZE * BLOCK_PER_CHUNK;
const BLOCK_PER_CHUNK: usize = 1024;
/// Default block size of 4 KiB
const BLOCK_SIZE: usize = 4096;

/// Chunk is the minimum storage unit with size of 4MiB.
pub struct Chunk {
    data: [RwLock<Block>; BLOCK_PER_CHUNK],
    subscriber: Mutex<Vec<Waker>>,
}

/// A Writer for `Chunk`
pub struct ChunkWriter<'a> {
    chunk: &'a Chunk,
    guards: [Option<RwLockWriteGuard<'a, Block>>; BLOCK_PER_CHUNK],
    ptr: usize,
}

/// A Reader for `Chunk`
pub struct ChunkReader<'a> {
    chunk: &'a Chunk,
    ptr: usize,
}

impl Chunk {
    /// Build a chunk with initialized blocks with zero.
    pub fn new() -> Self {
        let data: Box<[RwLock<Block>]> = (0..BLOCK_PER_CHUNK).map(|_| RwLock::new([0; BLOCK_SIZE])).collect();
        let data: Box<[RwLock<Block>; BLOCK_PER_CHUNK]> = data.try_into().unwrap();
        Self {
            data: *data,
            subscriber: Default::default()
        }
    }

    /// Build a chunk from exists blocks without copy it.
    /// Safety: the ownership is transferred to `RwLock` which won't leak the array.
    pub fn from_blocks(blocks: [Block; BLOCK_PER_CHUNK]) -> Self {
        let data: Box<[RwLock<Block>]> = unsafe {
            let blocks: [MaybeUninit<Block>; BLOCK_PER_CHUNK] = std::mem::transmute(blocks);
            (0..BLOCK_PER_CHUNK)
                .map(|i| {
                    let block = std::ptr::read(&blocks[i]).assume_init();
                    RwLock::new(block)
                })
                .collect()
        };
        let data: Box<[RwLock<Block>; BLOCK_PER_CHUNK]> = data.try_into().unwrap();
        Self {
            data: *data,
            subscriber: Default::default()
        }
    }

    pub fn writer(&self) -> ChunkWriter {
        ChunkWriter::new(self)
    }

    pub fn read(&self) -> ChunkReader {
        ChunkReader::new(self)
    }

    fn subscribe(&self, waker: Waker) {
        self.subscriber.lock().push(waker)
    }
}

impl<'a> ChunkWriter<'a> {
    pub fn new(chunk: &'a Chunk) -> Self {
        let guards: Box<[Option<RwLockWriteGuard<'a, Block>>]> =
            chunk.data.iter().map(|b| Some(b.write())).collect();
        let guards: Box<[Option<RwLockWriteGuard<'a, Block>>; BLOCK_PER_CHUNK]> =
            guards.try_into().unwrap();
        Self {
            chunk,
            guards: *guards,
            ptr: 0,
        }
    }

    fn write(&mut self, buf: &[u8], acc: usize) -> io::Result<usize> {
        // EOF
        if self.ptr == CHUNK_SIZE {
            return Ok(0);
        }
        let block_idx = self.ptr / BLOCK_SIZE;
        let offset = self.ptr % BLOCK_SIZE;
        debug_assert!(block_idx < BLOCK_PER_CHUNK);
        debug_assert!(offset < BLOCK_SIZE);

        let remaining = BLOCK_SIZE - offset;
        let write_in = min(remaining, buf.len());
        let guard = self.guards[block_idx].as_deref_mut().unwrap();
        guard[offset..offset + write_in].copy_from_slice(&buf[..write_in]);

        self.ptr += write_in;
        // drop used guard
        if self.ptr / BLOCK_SIZE != block_idx {
            self.guards[block_idx] = None;
        }

        // buf full
        if buf.len() == write_in {
            let mut subscriber = self.chunk.subscriber.lock();
            subscriber.iter().for_each(|s| s.wake_by_ref());
            subscriber.truncate(0);
            Ok(acc + write_in)
        } else {
            self.write(&buf[write_in..], acc + write_in)
        }
    }
}

impl<'a> ChunkReader<'a> {
    pub fn new(chunk: &'a Chunk) -> Self {
        Self {
            chunk,
            ptr: 0,
        }
    }

    /// Read the chunk, will block if there is no data available
    fn read(&mut self, buf: &mut [u8], acc: usize) -> io::Result<usize> {
        let n = self.try_read(buf, acc)?;
        // not EOF, data currently not available
        if n == 0 && self.ptr != CHUNK_SIZE {
            let block_idx = self.ptr / BLOCK_SIZE;
            debug_assert!(block_idx < BLOCK_PER_CHUNK);

            // block wait, this will immediately unlock after acquire
            #[allow(unused_must_use)]
            {
                self.chunk.data[block_idx].read_recursive();
            }

            self.try_read(buf, acc)
        } else {
            Ok(n)
        }
    }

    /// Try to read the chunk, will return Ok(0) if no data is available
    fn try_read(&mut self, buf: &mut [u8], acc: usize) -> io::Result<usize> {
        // EOF
        if self.ptr == CHUNK_SIZE {
            return Ok(acc);
        }

        let block_idx = self.ptr / BLOCK_SIZE;
        let offset = self.ptr % BLOCK_SIZE;
        debug_assert!(block_idx < BLOCK_PER_CHUNK);
        debug_assert!(offset < BLOCK_SIZE);

        if let Some(guard) = self.chunk.data[block_idx].try_read_recursive() {
            let remaining = BLOCK_SIZE - offset;
            let read_in = min(remaining, buf.len());
            buf[..read_in].copy_from_slice(&guard[offset..offset + read_in]);

            self.ptr += read_in;

            // buf full
            if buf.len() == read_in {
                Ok(acc + read_in)
            } else {
                self.try_read(&mut buf[read_in..], acc + read_in)
            }
        } else {
            Ok(acc)
        }
    }
}

impl<'a> Write for ChunkWriter<'a> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.write(buf, 0)
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl<'a> Read for ChunkReader<'a> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.read(buf, 0)
    }
}

impl<'a> AsyncWrite for ChunkWriter<'a> {
    fn poll_write(self: Pin<&mut Self>, _: &mut Context<'_>, buf: &[u8]) -> Poll<io::Result<usize>> {
        Poll::Ready(self.get_mut().write(buf, 0))
    }

    fn poll_flush(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<io::Result<()>> {
        Poll::Ready(Ok(()))
    }

    fn poll_shutdown(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<io::Result<()>> {
        let writer = self.get_mut();
        writer.guards
            .iter_mut()
            .filter(|block| block.is_some())
            .for_each(|block| *block = None);
        writer.ptr = CHUNK_SIZE;
        Poll::Ready(Ok(()))
    }
}

impl<'a> AsyncRead for ChunkReader<'a> {
    fn poll_read(self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &mut ReadBuf<'_>) -> Poll<io::Result<()>> {
        let reader = self.get_mut();
        // currently we cannot incrementally initialize `ReadBuf`
        match reader.try_read(buf.initialize_unfilled(), 0) {
            Ok(0) => {
                reader.chunk.subscribe(cx.waker().clone());
                Poll::Pending
            }
            Ok(n) => {
                buf.set_filled(buf.filled().len() + n);
                Poll::Ready(Ok(()))
            }
            Err(e) => Poll::Ready(Err(e))
        }

    }
}