use crate::{
  dynamic_import_usage::DynamicImportExportsUsage, ImportRecordIdx, Module, ModuleIdx,
  RawImportRecord, ResolvedId, SymbolRefDbForModule,
};
use minipack_ecmascript::EcmaAst;
use oxc_index::IndexVec;
use rustc_hash::FxHashMap;

pub struct NormalModuleTaskResult {
  pub ecma_related: Option<EcmaRelated>,
  pub module: Module,
  pub module_idx: ModuleIdx,
  pub resolved_deps: IndexVec<ImportRecordIdx, ResolvedId>,
  pub raw_import_records: IndexVec<ImportRecordIdx, RawImportRecord>,
  pub warnings: Vec<anyhow::Error>,
}

pub struct EcmaRelated {
  pub ast: EcmaAst,
  pub symbol_ref_db: SymbolRefDbForModule,
  pub dynamic_import_exports_usage: FxHashMap<ImportRecordIdx, DynamicImportExportsUsage>,
}
