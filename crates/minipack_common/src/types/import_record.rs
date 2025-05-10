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
    /// the import is inserted during ast transformation, can't get source slice from the original source file
    const IS_UNSPANNED_IMPORT = 1 << 1;
    /// `export * from 'mod'` only
    const IS_EXPORT_STAR = 1 << 2;
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

impl<State: Debug> ImportRecord<State> {
  pub fn is_unspanned(&self) -> bool {
    self.meta.contains(ImportRecordMeta::IS_UNSPANNED_IMPORT)
  }
}

impl RawImportRecord {
  pub fn new(specifier: Rstr, kind: ImportKind, namespace_ref: SymbolRef, span: Span) -> Self {
    Self { specifier, kind, namespace_ref, meta: ImportRecordMeta::empty(), state: span }
  }

  pub fn with_meta(mut self, meta: ImportRecordMeta) -> Self {
    self.meta = meta;
    self
  }

  pub fn into_resolved(self, resolved_module: ModuleIdx) -> ResolvedImportRecord {
    ResolvedImportRecord {
      state: resolved_module,
      specifier: self.specifier,
      kind: self.kind,
      namespace_ref: self.namespace_ref,
      meta: self.meta,
    }
  }
}
