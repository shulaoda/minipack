use minipack_common::{
  Chunk, ChunkKind, ModuleId, RenderedModule, RollupPreRenderedChunk, RollupRenderedChunk,
};
use rustc_hash::FxHashMap;

use crate::{graph::ChunkGraph, link_stage::LinkStageOutput};

use super::render_chunk_exports::get_chunk_export_names;

pub fn generate_rendered_chunk(
  chunk: &Chunk,
  render_modules: FxHashMap<ModuleId, RenderedModule>,
  pre_rendered_chunk: &RollupPreRenderedChunk,
  graph: &ChunkGraph,
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
    modules: render_modules.into(),
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

pub fn generate_pre_rendered_chunk(
  chunk: &Chunk,
  graph: &LinkStageOutput,
) -> RollupPreRenderedChunk {
  RollupPreRenderedChunk {
    name: chunk.name.clone().expect("should have name"),
    is_entry: matches!(&chunk.kind, ChunkKind::EntryPoint { is_user_defined, .. } if *is_user_defined),
    is_dynamic_entry: matches!(&chunk.kind, ChunkKind::EntryPoint { is_user_defined, .. } if !*is_user_defined),
    facade_module_id: match &chunk.kind {
      ChunkKind::EntryPoint { module, .. } => Some(graph.modules[*module].id().into()),
      ChunkKind::Common => None,
    },
    module_ids: chunk.modules.iter().map(|id| graph.modules[*id].id().into()).collect(),
    exports: get_chunk_export_names(chunk, graph),
  }
}
