use arcstr::ArcStr;
use bitflags::bitflags;
use minipack_utils::rstr::Rstr;
use oxc::span::Span;
use oxc_index::IndexVec;
use rustc_hash::FxHashMap;

use crate::{
  EcmaAstIdx, ImportRecordIdx, LocalExport, NamedImport, ResolvedImportRecord, StmtInfos,
  SymbolRef, side_effects::DeterminedSideEffects,
};

bitflags! {
    #[derive(Debug, Default)]
    pub struct EcmaViewMeta: u8 {
        const INCLUDED = 1;
        const HAS_STAR_EXPORT = 1 << 1;
    }
}

impl EcmaViewMeta {
  #[inline]
  pub fn is_included(&self) -> bool {
    self.contains(Self::INCLUDED)
  }

  #[inline]
  pub fn has_star_export(&self) -> bool {
    self.contains(Self::HAS_STAR_EXPORT)
  }
}

#[derive(Debug)]
pub struct EcmaView {
  pub source: ArcStr,
  pub ecma_ast_idx: Option<EcmaAstIdx>,
  /// Represents [Module Namespace Object](https://tc39.es/ecma262/#sec-module-namespace-exotic-objects)
  pub namespace_object_ref: SymbolRef,
  pub named_imports: FxHashMap<SymbolRef, NamedImport>,
  pub named_exports: FxHashMap<Rstr, LocalExport>,
  /// `stmt_infos[0]` represents the namespace binding statement
  pub stmt_infos: StmtInfos,
  pub import_records: IndexVec<ImportRecordIdx, ResolvedImportRecord>,
  /// The key is the `Span` of `ImportDeclaration`, `ImportExpression`, `ExportNamedDeclaration`, `ExportAllDeclaration`.
  pub imports: FxHashMap<Span, ImportRecordIdx>,
  pub default_export_ref: SymbolRef,
  pub side_effects: DeterminedSideEffects,
  pub meta: EcmaViewMeta,
}
