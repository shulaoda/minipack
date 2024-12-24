use minipack_common::{
  Chunk, ModuleId, RenderedModule, RollupPreRenderedChunk, RollupRenderedChunk,
};
use rustc_hash::FxHashMap;

use crate::graph::Graph;

pub fn generate_rendered_chunk(
  chunk: &Chunk,
  render_modules: FxHashMap<ModuleId, RenderedModule>,
  pre_rendered_chunk: &RollupPreRenderedChunk,
  graph: &Graph,
) -> RollupRenderedChunk {
  RollupRenderedChunk {
    name: pre_rendered_chunk.name.clone(),
    is_entry: pre_rendered_chunk.is_entry,
    is_dynamic_entry: pre_rendered_chunk.is_dynamic_entry,
    facade_module_id: pre_rendered_chunk.facade_module_id.clone(),
    module_ids: pre_rendered_chunk.module_ids.clone(),
    exports: pre_rendered_chunk.exports.clone(),
    filename: chunk
      .preliminary_filename
      .as_deref()
      .expect("should have preliminary_filename")
      .clone(),
    modules: render_modules,
    imports: chunk
      .cross_chunk_imports
      .iter()
      .map(|id| {
        graph.chunk_table[*id]
          .preliminary_filename
          .as_deref()
          .expect("should have preliminary_filename")
          .clone()
      })
      .collect(),
    dynamic_imports: chunk
      .cross_chunk_dynamic_imports
      .iter()
      .map(|id| {
        graph.chunk_table[*id]
          .preliminary_filename
          .as_deref()
          .expect("should have preliminary_filename")
          .clone()
      })
      .collect(),
    debug_id: 0,
  }
}
