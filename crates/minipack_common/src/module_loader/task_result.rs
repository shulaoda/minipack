use crate::{ImportRecordIdx, Module, RawImportRecord, ResolvedId, SymbolRefDbForModule};
use minipack_ecmascript::EcmaAst;
use oxc_index::IndexVec;

pub struct NormalModuleTaskResult {
  pub module: Module,
  pub ecma_related: Option<EcmaRelated>,
  pub resolved_deps: IndexVec<ImportRecordIdx, ResolvedId>,
  pub raw_import_records: IndexVec<ImportRecordIdx, RawImportRecord>,
  pub warnings: Vec<anyhow::Error>,
}

pub struct EcmaRelated {
  pub ast: EcmaAst,
  pub symbols: SymbolRefDbForModule,
}
