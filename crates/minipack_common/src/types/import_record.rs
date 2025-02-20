use std::{
  fmt::Debug,
  ops::{Deref, DerefMut},
};

use minipack_utils::rstr::Rstr;
use oxc::span::Span;

use crate::{ImportKind, ModuleIdx, ModuleType, StmtInfoIdx, SymbolRef, DUMMY_MODULE_IDX};

oxc_index::define_index_type! {
  pub struct ImportRecordIdx = u32;
}

#[derive(Debug)]
pub struct ImportRecordStateInit {
  pub span: Span,
  /// The importee of this import record is asserted to be this specific module type.
  pub asserted_module_type: Option<ModuleType>,
}

#[derive(Debug)]
pub struct ImportRecordStateResolved {
  pub resolved_module: ModuleIdx,
}

bitflags::bitflags! {
  #[derive(Debug)]
  pub struct ImportRecordMeta: u8 {
    /// If it is `import * as ns from '...'` or `export * as ns from '...'`
    const CONTAINS_IMPORT_STAR = 1;
    /// If it is `import def from '...'`, `import { default as def }`, `export { default as def }` or `export { default } from '...'`
    const CONTAINS_IMPORT_DEFAULT = 1 << 1;
    /// If it is `import {} from '...'` or `import '...'`
    const IS_PLAIN_IMPORT = 1 << 2;
    /// the import is inserted during ast transformation, can't get source slice from the original source file
    const IS_UNSPANNED_IMPORT = 1 << 3;
    /// `export * from 'mod'` only
    const IS_EXPORT_STAR = 1 << 4;
    ///  Tell the finalizer to use the runtime "__require()" instead of "require()"
    const CALL_RUNTIME_REQUIRE = 1 << 5;
    ///  `require('mod')` is used to load the module only
    const IS_REQUIRE_UNUSED = 1 << 6;
    /// If the import is a dummy import, it should be ignored during linking, e.g.
    /// `require` ExpressionIdentifier should be considering as a import record,
    /// but it did not import any module.
    const IS_DUMMY = 1 << 7;
  }
}

#[derive(Debug)]
pub struct ImportRecord<State: Debug> {
  pub state: State,
  /// `./lib.js` in `import { foo } from './lib.js';`
  pub module_request: Rstr,
  pub kind: ImportKind,
  /// We will turn `import { foo } from './cjs.js'; console.log(foo);` to `var import_foo = require_cjs(); console.log(importcjs.foo)`;
  /// `namespace_ref` represent the potential `import_foo` in above example. It's useless if we imported n esm module.
  pub namespace_ref: SymbolRef,
  pub meta: ImportRecordMeta,
  pub related_stmt_info_idx: Option<StmtInfoIdx>,
}

impl<State: Debug> ImportRecord<State> {
  pub fn is_unspanned(&self) -> bool {
    self.meta.contains(ImportRecordMeta::IS_UNSPANNED_IMPORT)
  }
}

impl<T: Debug> Deref for ImportRecord<T> {
  type Target = T;

  fn deref(&self) -> &Self::Target {
    &self.state
  }
}

impl<T: Debug> DerefMut for ImportRecord<T> {
  fn deref_mut(&mut self) -> &mut Self::Target {
    &mut self.state
  }
}

pub type RawImportRecord = ImportRecord<ImportRecordStateInit>;

impl RawImportRecord {
  pub fn new(
    specifier: Rstr,
    kind: ImportKind,
    namespace_ref: SymbolRef,
    span: Span,
    asserted_module_type: Option<ModuleType>,
    related_stmt_info_idx: Option<StmtInfoIdx>,
  ) -> RawImportRecord {
    RawImportRecord {
      module_request: specifier,
      kind,
      namespace_ref,
      meta: ImportRecordMeta::empty(),
      state: ImportRecordStateInit { span, asserted_module_type },
      related_stmt_info_idx,
    }
  }

  pub fn with_meta(mut self, meta: ImportRecordMeta) -> Self {
    self.meta = meta;
    self
  }

  pub fn into_resolved(self, resolved_module: ModuleIdx) -> ResolvedImportRecord {
    ResolvedImportRecord {
      state: ImportRecordStateResolved { resolved_module },
      module_request: self.module_request,
      kind: self.kind,
      namespace_ref: self.namespace_ref,
      meta: self.meta,
      related_stmt_info_idx: self.related_stmt_info_idx,
    }
  }
}

pub type ResolvedImportRecord = ImportRecord<ImportRecordStateResolved>;

impl ResolvedImportRecord {
  pub fn is_dummy(&self) -> bool {
    self.state.resolved_module == DUMMY_MODULE_IDX
  }
}
