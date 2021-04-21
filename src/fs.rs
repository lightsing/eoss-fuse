
pub const ID_LENGTH: usize = 64;

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
    Unknown,
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
