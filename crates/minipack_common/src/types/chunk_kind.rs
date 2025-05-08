use crate::ModuleIdx;

#[derive(Debug, Default)]
pub enum ChunkKind {
  EntryPoint { is_user_defined: bool, bit: u32, module: ModuleIdx },
  #[default]
  Common,
}