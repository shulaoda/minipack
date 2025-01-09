use crate::{Module, ModuleIdx};
use oxc_index::IndexVec;

pub type IndexModules = IndexVec<ModuleIdx, Module>;

#[derive(Debug, Default)]
pub struct ModuleTable {
  pub modules: IndexModules,
}
