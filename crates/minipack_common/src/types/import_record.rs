use std::fmt::Debug;

use minipack_utils::rstr::Rstr;
use oxc::span::Span;

use crate::{ImportKind, ModuleIdx, SymbolRef};

pub type RawImportRecord = ImportRecord<Span>;
pub type ResolvedImportRecord = ImportRecord<ModuleIdx>;

bitflags::bitflags! {
  #[derive(Debug)]
  pub struct ImportRecordMeta: u8 {
    /// If it is `import {} from '...'` or `import '...'`
    const IS_PLAIN_IMPORT = 1;
    /// `export * from 'mod'` only
    const IS_EXPORT_STAR = 1 << 1;
  }
}

#[derive(Debug)]
pub struct ImportRecord<State: Debug> {
  pub state: State,
  /// `./lib.js` in `import { foo } from './lib.js';`
  pub specifier: Rstr,
  pub kind: ImportKind,
  pub meta: ImportRecordMeta,
  pub namespace_ref: SymbolRef,
}

impl RawImportRecord {
  pub fn new(specifier: Rstr, kind: ImportKind, namespace_ref: SymbolRef, span: Span) -> Self {
    Self { specifier, kind, namespace_ref, meta: ImportRecordMeta::empty(), state: span }
  }

  pub fn with_meta(mut self, meta: ImportRecordMeta) -> Self {
    self.meta = meta;
    self
  }

  pub fn into_resolved(self, module_idx: ModuleIdx) -> ResolvedImportRecord {
    ResolvedImportRecord {
      state: module_idx,
      kind: self.kind,
      meta: self.meta,
      specifier: self.specifier,
      namespace_ref: self.namespace_ref,
    }
  }
}
