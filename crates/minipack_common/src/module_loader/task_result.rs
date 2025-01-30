use crate::{
  dynamic_import_usage::DynamicImportExportsUsage, ImportRecordIdx, Module, RawImportRecord,
  ResolvedId, SymbolRefDbForModule,
};
use minipack_ecmascript::EcmaAst;
use oxc_index::IndexVec;
use rustc_hash::FxHashMap;

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
  pub dynamic_import_exports_usage: FxHashMap<ImportRecordIdx, DynamicImportExportsUsage>,
}
