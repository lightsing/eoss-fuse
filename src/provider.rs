use crate::chunk::{Chunk, ChunkError};
use crate::id::Id;

#[derive(thiserror::Error, Debug)]
pub enum ChunkProviderError {
    #[error(transparent)]
    IoError(#[from] std::io::Error),
    #[error(transparent)]
    ChunkError(#[from] ChunkError),
}

pub trait ChunkProvider: Send + Sync {
    /// Request a chunk from the provider with chunk id
    fn get_chunk_by_id(&self, id: &Id) -> Result<Chunk, ChunkProviderError>;
    /// Request a list of chunks from the provider with chunk id
    fn get_chunk_by_ids(&self, ids: &[&Id]) -> Result<Vec<Chunk>, ChunkProviderError> {
        ids
            .iter()
            .map(|id| self.get_chunk_by_id(id))
            .collect()
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
