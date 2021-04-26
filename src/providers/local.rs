use std::io;
use std::fs;
use std::path::{Path, PathBuf};

use crate::provider::{ChunkProvider, ChunkProviderError};
use crate::chunk::{Chunk, CHUNK_SIZE};
use crate::id::Id;

pub struct LocalProvider {
    base: PathBuf,
}

impl LocalProvider {
    pub fn new<P: AsRef<Path>>(path: P) -> io::Result<Self> {
        if !path.as_ref().exists() {
            fs::create_dir_all(&path)?;
        }
        if !path.as_ref().is_dir() {
            return Err(io::Error::from(io::ErrorKind::AlreadyExists))
        }
        Ok(LocalProvider {
            base: path.as_ref().to_owned()
        })
    }

    fn get_path(&self, id: &Id) -> PathBuf {
        let file_name = id.hex();
        let mut path = self.base.clone();
        path.push(&file_name[..2]);
        path.push(&file_name[..4]);
        path.push(&file_name[..6]);
        path.push(file_name);
        path
    }
}

impl ChunkProvider for LocalProvider {
    fn get_chunk_by_id(&self, id: &Id) -> Result<Chunk, ChunkProviderError> {
        let path = self.get_path(id);
        if path.exists() {
            let data = fs::read(path)?;
            Ok(Chunk::new_with_data(id.clone(), data)?)
        } else {
            let file = fs::File::create(path)?;
            file.set_len(CHUNK_SIZE as u64)?;
            Ok(Chunk::new(id.clone()))
        }
    }

    fn save_chunk(&self, chunk: &Chunk) -> Result<(), ChunkProviderError> {
        let path = self.get_path(chunk.id());
        let mut file = fs::OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .open(path)?;
        let mut reader = chunk.read();
        io::copy(&mut reader, &mut file)?;
        Ok(())
    }
}
