use crate::{ImportKind, ModuleId};

#[derive(Debug)]
pub struct ImporterRecord {
  pub kind: ImportKind,
  pub importer_path: ModuleId,
}
