use fuser::{Filesystem, Request};
use bitmaps::Bitmap;
use typenum::U1024;
use crate::provider::ChunkProvider;
use std::io::{self, Read, Write};
use parking_lot::{RwLock, RwLockWriteGuard};
use std::convert::TryInto;
use std::cmp::{max, min};

pub const ID_LENGTH: usize = 64;
const CHUNK_SIZE: usize = BLOCK_SIZE * BLOCK_PER_CHUNK;
const BLOCK_PER_CHUNK: usize = 1024;
/// Default block size of 4 KiB
const BLOCK_SIZE: usize = 4096;

pub type Block = [u8; BLOCK_SIZE];
/// Chunk is the minimum storage unit with size of 4MiB.
pub struct Chunk {
    data: [RwLock<Block>; BLOCK_PER_CHUNK],
}

pub struct ChunkWriter<'a> {
    guards: [RwLockWriteGuard<'a, Block>; BLOCK_PER_CHUNK],
    ptr: usize,
    available: usize,
}


impl Chunk {
    pub fn writer(&self) -> ChunkWriter {
        ChunkWriter::new(self)
    }
}

impl <'a> ChunkWriter<'a> {
    pub fn new(chunk: &'a Chunk) -> Self {
        let guards: Box<[RwLockWriteGuard<'a, Block>]> = chunk.data.iter().map(|b| b.write()).collect();
        let guards: Box<[RwLockWriteGuard<'a, Block>; BLOCK_PER_CHUNK]> = guards.try_into().unwrap();
        Self {
            guards: *guards,
            ptr: 0,
            available: BLOCK_PER_CHUNK * BLOCK_SIZE
        }
    }
}

impl Read for Chunk {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        todo!()
    }
}

impl <'a> ChunkWriter<'a> {
    fn write(&mut self, buf: &[u8], acc: uszie) -> io::Result<uszie> {
        let block_idx = self.ptr / BLOCK_SIZE;
        let offset = self.ptr % BLOCK_SIZE;
        debug_assert!(block_idx < BLOCK_PER_CHUNK);
        debug_assert!(offset < BLOCK_SIZE);

        let remaining = BLOCK_SIZE - offset;
        let write_in = min(remaining, buf.len());
        self.guards[block_idx][offset..offset + write_in].copy_from_slice(&buf[..write_in]);

        self.ptr += write_in;

        if buf.len() == write_in {
            Ok(acc + write_in)
        } else {
            self.write(&buf[..write_in], acc + write_in)
        }
    }
}

impl <'a> Write for ChunkWriter<'a> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.write(buf, 0)
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}


pub struct RawChunk;
pub struct MetaChunk;
pub struct TinyFileChunk;

enum ChunkType {
    /// Chunk holds raw data (file part)
    Raw,
    /// Chunk holds a directory metadata
    DirMeta,
    /// Chunk holds tiny files
    TinyFiles,
    /// If a provider does not known the type, leave it as Unknown
    Unknown
}

impl Chunk {
    fn get_type(&self) -> ChunkType {
        ChunkType::Raw
    }
}

struct TinyFileChunkMeta<'a> {
    data: &'a [u8; 20],
}

/// FileMeta stores the metadata of a file with size greater or
/// equal than 4MiB.
/// A file with size greater or equal than 4MiB SHOULD be store
/// in multiple *contiguous* exclusive chunks.
struct FileMeta {
    id: [u8; ID_LENGTH],
    /// POSIX attributes contains the size and blocks of this file.
    attrs: Attrs,
}
/// TinyFileMeta stores the metadata of a file with size less than
/// 4MiB.
/// A file with size less than 4MiB MAY be stored with in
/// a shared chunk.
struct TinyFileMeta {
    id: [u8; ID_LENGTH],
    chunk_id: [u8; ID_LENGTH],
    chunk_offset: u16,
    /// POSIX attributes contains the size and blocks of this file.
    attrs: Attrs,
}

/// DirMeta stores the metadata of a directory.
/// A directory may contains 0 or more sub-directories.
/// A directory may contains 0 or more files.
struct DirMeta {
    dirs: Vec<DirMeta>,
    files: Vec<FileMeta>,
    tiny_files: Vec<TinyFileMeta>,
    attrs: Attrs,
}

/// Attrs contains all needed POSIX attributes
struct Attrs {
    size: u64,
    blocks: u64,
    // ... more POSIX attributes
}
