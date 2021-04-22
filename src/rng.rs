use crossbeam::queue::ArrayQueue;
use once_cell::sync::Lazy;
use rand::rngs::StdRng;
use rand::{thread_rng, SeedableRng};
use std::ops::{Deref, DerefMut};

static RNG_POOL: Lazy<ArrayQueue<StdRng>> = Lazy::new(|| ArrayQueue::new(10));

pub struct Rng {
    inner: Option<StdRng>,
}

impl Rng {
    pub fn new() -> Self {
        Self {
            inner: Some(
                RNG_POOL
                    .pop()
                    .unwrap_or_else(|| StdRng::from_rng(thread_rng()).unwrap()),
            ),
        }
    }
}

impl Deref for Rng {
    type Target = StdRng;

    fn deref(&self) -> &Self::Target {
        self.inner.as_ref().unwrap()
    }
}

impl DerefMut for Rng {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.inner.as_mut().unwrap()
    }
}

impl Drop for Rng {
    fn drop(&mut self) {
        RNG_POOL.push(self.inner.take().unwrap()).ok();
    }
}
