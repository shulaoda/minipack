use arcstr::ArcStr;
use oxc_index::IndexVec;

use crate::side_effects::DeterminedSideEffects;
use crate::{ImportRecordIdx, ModuleIdx, ResolvedImportRecord, SymbolRef};

#[derive(Debug)]
pub struct ExternalModule {
  pub idx: ModuleIdx,
  pub name: ArcStr,
  pub exec_order: u32,
  /// Usages:
  /// - Used for iife format to inject symbol and deconflict.
  /// - Used for for rewrite `import { foo } from 'external';console.log(foo)` to `var external = require('external'); console.log(external.foo)` in cjs format.
  pub namespace_ref: SymbolRef,
  pub import_records: IndexVec<ImportRecordIdx, ResolvedImportRecord>,
  pub side_effects: DeterminedSideEffects,
}

impl ExternalModule {
  pub fn new(
    idx: ModuleIdx,
    name: ArcStr,
    namespace_ref: SymbolRef,
  ) -> Self {
    Self {
      idx,
      name,
      namespace_ref,
      exec_order: u32::MAX,
      import_records: IndexVec::default(),
      side_effects: DeterminedSideEffects::NoTreeshake,
    }
  }
}
