use arcstr::ArcStr;

use crate::{ModuleIdx, StmtInfoIdx};

#[derive(Debug, Hash, PartialEq, Eq, Clone)]
pub struct EntryPoint {
  pub id: ModuleIdx,
  pub name: Option<ArcStr>,
  pub kind: EntryPointKind,
  pub related_stmt_infos: Vec<(ModuleIdx, StmtInfoIdx)>,
}

#[derive(Debug, Hash, PartialEq, Eq, Clone, Copy)]
pub enum EntryPointKind {
  UserDefined,
  DynamicImport,
}

impl EntryPointKind {
  pub fn is_user_defined(&self) -> bool {
    matches!(self, Self::UserDefined)
  }
}
