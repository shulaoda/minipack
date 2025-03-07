use minipack_common::{
  ChunkIdx, ModuleIdx, NormalModule, RuntimeModuleBrief, SymbolRef, SymbolRefDb,
};
use minipack_utils::rstr::Rstr;
use rustc_hash::FxHashMap;

use crate::{
  graph::ChunkGraph,
  types::{
    IndexModules, SharedOptions,
    linking_metadata::{LinkingMetadata, LinkingMetadataVec},
  },
};

pub struct ScopeHoistingFinalizerContext<'me> {
  pub id: ModuleIdx,
  pub chunk_id: ChunkIdx,
  pub module: &'me NormalModule,
  pub modules: &'me IndexModules,
  pub linking_info: &'me LinkingMetadata,
  pub linking_infos: &'me LinkingMetadataVec,
  pub symbol_db: &'me SymbolRefDb,
  pub canonical_names: &'me FxHashMap<SymbolRef, Rstr>,
  pub runtime: &'me RuntimeModuleBrief,
  pub chunk_graph: &'me ChunkGraph,
  pub options: &'me SharedOptions,
  pub cur_stmt_index: usize,
  pub keep_name_statement_to_insert: Vec<(usize, Rstr, Rstr)>,
}
