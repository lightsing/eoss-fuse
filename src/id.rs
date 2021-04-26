use std::cmp::Ordering;
use std::fmt;
use std::fmt::Formatter;
use std::hash::{Hash, Hasher};
use std::ops::Deref;
use std::sync::Arc;

use once_cell::sync::OnceCell;
use rand::{RngCore, thread_rng};

pub const ID_LENGTH: usize = 32;

#[derive(Clone, Debug, Eq)]
pub struct Id {
    inner: Arc<[u8; ID_LENGTH]>,
    hex: OnceCell<Box<str>>,
}

impl Id {
    pub fn new(id: [u8; ID_LENGTH]) -> Self {
        let inner = Arc::new(id);
        Self {
            inner,
            hex: OnceCell::new(),
        }
    }

    pub fn new_random() -> Self {
        let mut id = [0u8; ID_LENGTH];
        thread_rng().fill_bytes(&mut id);
        Self::new(id)
    }

    pub fn hex(&self) -> &str {
        self.hex
            .get_or_init(|| hex::encode(&*self.inner).into_boxed_str())
    }

    pub fn derive_n(&self, n: usize) -> [u8; ID_LENGTH] {
        let hash = blake3::keyed_hash(self.inner.deref(), n.to_le_bytes().as_ref());
        *hash.as_bytes()
    }
}

impl Deref for Id {
    type Target = [u8; ID_LENGTH];

    fn deref(&self) -> &Self::Target {
        self.inner.as_ref()
    }
}

impl AsRef<[u8]> for Id {
    fn as_ref(&self) -> &[u8] {
        &self.inner.deref()[..]
    }
}

impl PartialEq for Id {
    fn eq(&self, other: &Self) -> bool {
        self.inner.eq(&other.inner)
    }
}

impl Hash for Id {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.inner.hash(state)
    }
}

impl PartialOrd for Id {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.inner.partial_cmp(&other.inner)
    }
}

impl Ord for Id {
    fn cmp(&self, other: &Self) -> Ordering {
        self.inner.cmp(&other.inner)
    }
}

impl fmt::Display for Id {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "Id({})", self.hex())
    }
}

impl From<[u8; ID_LENGTH]> for Id {
    fn from(inner: [u8; ID_LENGTH]) -> Self {
        Self::new(inner)
    }
}


#[cfg(test)]
mod tests {
    use crate::id::Id;

    #[test]
    fn test_random() {
        let id = Id::new_random();
        println!("{}", id);
    }
}
