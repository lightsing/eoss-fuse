# Chapter 1

## Basic Concepts

### Chunk

`Chunk` is a fixed length file stored in oss backend. 
`Chunk` is the minimum storage unit of EOSS-fs.

A typical chunk size is `4MiB`.

### Block

`Block` is a fixed length slice of `Chunk` aligned to block size.

A typical block size is `4KiB`, thus a typical chunk has 1024 blocks.

### File

`File` corresponds to a file with size greater or equal than `4MiB`.
A `File` is stored in a list of exclusive chunks except for the last chunk.
Except for the last chunk, chunks used by `File` are `RawChunk`.
Last chunk will be `SharedChunk` unless it cannot fit in a `SharedChunk`.

Each `File` hash a unique id of `32 Bytes` random generated.

### TinyFile

`TinyFile` corresponds to a file with size less than `4MiB`.
A `TinyFile` is stored in a `SharedChunk`.

Each `File` hash a unique id of `32 Bytes` random generated.

### RawChunk

`RawChunk` is a chunk filled with data.

### SharedChunk

`SharedChunk` is a chunk shared by one or more `TinyFile`s and
at most one `File`.
A `FatBitMap` takes the last 2 blocks of a `SharedChunk`.

### MetaChunk

`MetaChunk` is a chunk holds `Dir` metadata.