use crate::provider::{ChunkProvider, ChunkProviderError};
use crate::chunk::Chunk;
use crate::id::Id;

struct LocalProvider;

impl ChunkProvider for LocalProvider {
    fn get_chunk_by_id(&self, id: &Id) -> Option<Chunk> {
        todo!()
    }

    fn save_chunk(&self, chunk: &Chunk) -> Result<(), ChunkProviderError> {
        todo!()
    }
}