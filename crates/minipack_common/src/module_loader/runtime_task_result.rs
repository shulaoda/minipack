use minipack_ecmascript::EcmaAst;
use oxc_index::IndexVec;

use crate::{ImportRecordIdx, NormalModule, RawImportRecord, ResolvedId, SymbolRefDbForModule};

use super::runtime_module_brief::RuntimeModuleBrief;

pub struct RuntimeModuleTaskResult {
  pub ast: EcmaAst,
  pub module: NormalModule,
  pub local_symbol_ref_db: SymbolRefDbForModule,
  pub runtime: RuntimeModuleBrief,
  pub resolved_deps: IndexVec<ImportRecordIdx, ResolvedId>,
  pub raw_import_records: IndexVec<ImportRecordIdx, RawImportRecord>,
}
