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
  pub meta: EcmaViewMeta,
  pub ecma_ast_idx: Option<EcmaAstIdx>,
  pub stmt_infos: StmtInfos,
  pub side_effects: DeterminedSideEffects,
  pub default_export_ref: SymbolRef,
  pub namespace_object_ref: SymbolRef,
  pub named_imports: FxHashMap<SymbolRef, NamedImport>,
  pub named_exports: FxHashMap<Rstr, LocalExport>,
  pub imports: FxHashMap<Span, ImportRecordIdx>,
  pub import_records: IndexVec<ImportRecordIdx, ResolvedImportRecord>,
}
