use std::cmp::min;
use std::convert::TryInto;
use std::io::{self, Read, Write};
use parking_lot::{RwLock, RwLockWriteGuard};

pub type Block = [u8; BLOCK_SIZE];

const CHUNK_SIZE: usize = BLOCK_SIZE * BLOCK_PER_CHUNK;
const BLOCK_PER_CHUNK: usize = 1024;
/// Default block size of 4 KiB
const BLOCK_SIZE: usize = 4096;

/// Chunk is the minimum storage unit with size of 4MiB.
pub struct Chunk {
    data: [RwLock<Block>; BLOCK_PER_CHUNK],
}

pub struct ChunkWriter<'a> {
    guards: [Option<RwLockWriteGuard<'a, Block>>; BLOCK_PER_CHUNK],
    ptr: usize,
}

pub struct ChunkReader<'a> {
    chunk: &'a [RwLock<Block>; BLOCK_PER_CHUNK],
    ptr: usize,
}

impl Chunk {
    pub fn writer(&self) -> ChunkWriter {
        ChunkWriter::new(self)
    }

    pub fn read(&self) -> ChunkReader {
        ChunkReader::new(self)
    }
}

impl<'a> ChunkWriter<'a> {
    pub fn new(chunk: &'a Chunk) -> Self {
        let guards: Box<[Option<RwLockWriteGuard<'a, Block>>]> =
            chunk.data.iter().map(|b| Some(b.write())).collect();
        let guards: Box<[Option<RwLockWriteGuard<'a, Block>>; BLOCK_PER_CHUNK]> =
            guards.try_into().unwrap();
        Self {
            guards: *guards,
            ptr: 0,
        }
    }

    fn write(&mut self, buf: &[u8], acc: usize) -> io::Result<usize> {
        // EOF
        if self.ptr == BLOCK_SIZE * BLOCK_PER_CHUNK {
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
            Ok(acc + write_in)
        } else {
            self.write(&buf[write_in..], acc + write_in)
        }
    }
}

impl<'a> ChunkReader<'a> {
    pub fn new(chunk: &'a Chunk) -> Self {
        Self {
            chunk: &chunk.data,
            ptr: 0,
        }
    }

    fn read(&mut self, buf: &mut [u8], acc: usize) -> io::Result<usize> {
        // EOF
        if self.ptr == BLOCK_SIZE * BLOCK_PER_CHUNK {
            return Ok(acc);
        }
        let block_idx = self.ptr / BLOCK_SIZE;
        let offset = self.ptr % BLOCK_SIZE;
        debug_assert!(block_idx < BLOCK_PER_CHUNK);
        debug_assert!(offset < BLOCK_SIZE);

        // ensure first
        let guard = if acc == 0 {
            self.chunk[block_idx].read_recursive()
        } else {
            match self.chunk[block_idx].try_read_recursive() {
                None => return Ok(acc),
                Some(guard) => guard,
            }
        };

        let remaining = BLOCK_SIZE - offset;
        let read_in = min(remaining, buf.len());
        buf[..read_in].copy_from_slice(&guard[offset..offset + read_in]);

        self.ptr += read_in;

        // buf full
        if buf.len() == read_in {
            Ok(acc + read_in)
        } else {
            self.read(&mut buf[read_in..], acc + read_in)
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