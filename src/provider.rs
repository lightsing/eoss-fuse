use crate::fs::{Chunk, ID_LENGTH};

#[derive(Debug)]
pub enum ChunkProviderError {}

pub trait ChunkProvider {
    /// Request a chunk from the provider with chunk id
    fn get_chunk_by_id(&self, id: &[u8; ID_LENGTH]) -> Option<Chunk>;
    /// Request a list of chunks from the provider with chunk id
    fn get_chunk_by_ids(&self, ids: &[&[u8; ID_LENGTH]]) -> Vec<Option<Chunk>> {
        let mut chunks = Vec::with_capacity(ids.len());
        for id in ids {
            chunks.push(self.get_chunk_by_id(id))
        }
        chunks
    }
    /// Save modifications of a chunk, create if not exists
    fn save_chunk(&self, chunk: &Chunk) -> Result<(), ChunkProviderError>;
    /// Save modifications of all chunks, create if not exists
    fn save_all_chunks(&self, chunks: &[&Chunk]) -> Result<(), ChunkProviderError> {
        for chunk in chunks {
            self.save_chunk(chunk)?
        }
        Ok(())
    }
    /// Request the provider to flush all cached writes.
    fn flush(&self) -> Result<(), ChunkProviderError> {
        Ok(())
    }
}