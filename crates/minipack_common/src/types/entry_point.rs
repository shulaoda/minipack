use arcstr::ArcStr;

use crate::ModuleIdx;

#[derive(Debug)]
pub struct EntryPoint {
  pub idx: ModuleIdx,
  pub name: Option<ArcStr>,
  pub kind: EntryPointKind,
}

#[derive(Debug, PartialEq, Eq)]
pub enum EntryPointKind {
  UserDefined,
  DynamicImport,
}
