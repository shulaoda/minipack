use std::ops::{Deref, DerefMut};

use oxc_index::IndexVec;

use crate::ChunkIdx;

use super::Chunk;

#[derive(Debug, Default)]
pub struct ChunkTable {
  pub chunks: IndexVec<ChunkIdx, Chunk>,
}

impl Deref for ChunkTable {
  type Target = IndexVec<ChunkIdx, Chunk>;

  fn deref(&self) -> &Self::Target {
    &self.chunks
  }
}

impl DerefMut for ChunkTable {
  fn deref_mut(&mut self) -> &mut Self::Target {
    &mut self.chunks
  }
}

impl ChunkTable {
  pub fn new(chunks: IndexVec<ChunkIdx, Chunk>) -> Self {
    Self { chunks }
  }
}
