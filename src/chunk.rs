use std::cmp::min;
use std::convert::TryInto;
use std::io::{self, Read, Write, Seek, SeekFrom};
use parking_lot::{RwLock, RwLockWriteGuard, Mutex};
use tokio::io::{AsyncWrite, AsyncRead, ReadBuf, AsyncSeek};
use std::task::{Context, Poll, Waker};
use std::pin::Pin;
use std::ops::{Deref, DerefMut};
use std::array;
use crate::fs::ID_LENGTH;

const CHUNK_SIZE: usize = BLOCK_SIZE * BLOCK_PER_CHUNK;
const BLOCK_PER_CHUNK: usize = 1024;
/// Default block size of 4 KiB
const BLOCK_SIZE: usize = 4096;

#[derive(Clone, Debug)]
pub struct Block(Box<[u8; BLOCK_SIZE]>);

impl Default for Block {
    fn default() -> Self {
        Self(Box::new([0; BLOCK_SIZE]))
    }
}

impl Deref for Block {
    type Target = [u8; BLOCK_SIZE];

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for Block {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

/// Chunk is the minimum storage unit with size of 4MiB.
pub struct Chunk {
    id: [u8; ID_LENGTH],
    data: Box<[RwLock<Block>; BLOCK_PER_CHUNK]>,
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
    pub fn new(id: [u8; ID_LENGTH]) -> Self {
        let data: Box<[RwLock<Block>]> = (0..BLOCK_PER_CHUNK).map(|_| RwLock::new(Block::default())).collect();
        let data: Box<[RwLock<Block>; BLOCK_PER_CHUNK]> = data.try_into().unwrap();
        Self {
            id,
            data,
            subscriber: Default::default()
        }
    }

    /// Build a chunk from exists blocks without copy its content.
    pub fn new_with_blocks(id: [u8; ID_LENGTH], blocks: [Block; BLOCK_PER_CHUNK]) -> Self {
        let data: Box<[RwLock<Block>]> =
            array::IntoIter::new(blocks)
                .map(RwLock::new)
                .collect();
        Self {
            id,
            data: data.try_into().unwrap(),
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

/// Build blocks from a existing chunk without copy its content.
impl From<Chunk> for Box<[Block; BLOCK_PER_CHUNK]> {
    fn from(chunk: Chunk) -> Self {
        array::IntoIter::new(*chunk.data)
            .map(|l| l.into_inner())
            .collect::<Box<[Block]>>()
            .try_into().unwrap()
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

impl<'a> Seek for ChunkReader<'a> {
    fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
        let target = match pos {
            SeekFrom::Start(offset) => offset as i64,
            SeekFrom::End(offset) => CHUNK_SIZE as i64 + offset,
            SeekFrom::Current(offset) => self.ptr as i64 + offset
        };
        if target < 0 {
            Err(io::Error::from(io::ErrorKind::InvalidInput))
        } else if target as usize >= CHUNK_SIZE {
            self.ptr = CHUNK_SIZE - 1;
            Ok((CHUNK_SIZE - 1) as u64)
        } else {
            self.ptr = target as usize;
            Ok(target as u64)
        }
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

impl<'a> AsyncSeek for ChunkReader<'a> {
    fn start_seek(self: Pin<&mut Self>, position: SeekFrom) -> io::Result<()> {
        self.get_mut().seek(position)?;
        Ok(())
    }

    fn poll_complete(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<io::Result<u64>> {
        Poll::Ready(Ok(self.ptr as u64))
    }
}

#[cfg(test)]
mod tests {
    use super::{Chunk, BLOCK_SIZE, BLOCK_PER_CHUNK};
    use std::io::Write;

    #[test]
    fn test_write() {
        let chunk = Chunk::new();
        let mut chunk_writer = chunk.writer();
        for i in 0..BLOCK_PER_CHUNK / 4 {
            chunk_writer.write_all(&[i as u8; 4096 * 4]).unwrap();
        }
        drop(chunk_writer);
        for i in 0..BLOCK_PER_CHUNK {
            for j in 0..BLOCK_SIZE {
                assert_eq!(chunk.data[i].read()[j], (i / 4) as u8)
            }
        }
    }
}