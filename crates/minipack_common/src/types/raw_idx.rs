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