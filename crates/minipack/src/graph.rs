use minipack_common::{Chunk, ChunkIdx, ModuleIdx};
use oxc_index::{index_vec, IndexVec};
use rustc_hash::FxHashMap;

use crate::types::IndexModules;

#[derive(Debug)]
pub struct ChunkGraph {
  pub chunk_table: IndexVec<ChunkIdx, Chunk>,
  pub sorted_chunk_idx_vec: Vec<ChunkIdx>,
  pub module_to_chunk: IndexVec<ModuleIdx, Option<ChunkIdx>>,
  pub entry_module_to_entry_chunk: FxHashMap<ModuleIdx, ChunkIdx>,
}

impl ChunkGraph {
  pub fn new(modules: &IndexModules) -> Self {
    Self {
      chunk_table: IndexVec::default(),
      module_to_chunk: index_vec![None; modules.len()],
      sorted_chunk_idx_vec: Vec::new(),
      entry_module_to_entry_chunk: FxHashMap::default(),
    }
  }

  pub fn add_chunk(&mut self, chunk: Chunk) -> ChunkIdx {
    self.chunk_table.push(chunk)
  }

  pub fn add_module_to_chunk(&mut self, module_idx: ModuleIdx, chunk_idx: ChunkIdx) {
    self.chunk_table[chunk_idx].modules.push(module_idx);
    self.module_to_chunk[module_idx] = Some(chunk_idx);
  }
}
