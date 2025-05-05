oxc_index::define_index_type! {
  #[derive(Default)]
  pub struct RawIdx = u32;
}

pub type AssetIdx = RawIdx;
pub type ChunkIdx = RawIdx;
pub type ModuleIdx = RawIdx;
pub type EcmaAstIdx = RawIdx;
pub type StmtInfoIdx = RawIdx;
pub type ImportRecordIdx = RawIdx;

// Preserved module idx used for representing a module that is not in the module graph.
// e.g.
// create a module idx for `ImportRecord` ExpressionIdentifier
pub const DUMMY_MODULE_IDX: RawIdx = ModuleIdx::from_usize_unchecked(u32::MAX as usize);
