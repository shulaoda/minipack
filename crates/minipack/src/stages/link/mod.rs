use minipack_common::{
  dynamic_import_usage::DynamicImportExportsUsage, EntryPoint, ModuleIdx, ModuleTable,
  RuntimeModuleBrief, SymbolRef, SymbolRefDb,
};
use minipack_error::BuildError;
use rustc_hash::{FxHashMap, FxHashSet};

use crate::types::{linking_metadata::LinkingMetadataVec, IndexEcmaAst, SharedOptions};

use super::scan::ScanStageOutput;

#[derive(Debug)]
pub struct LinkStageOutput {
  pub module_table: ModuleTable,
  pub entries: Vec<EntryPoint>,
  pub ast_table: IndexEcmaAst,
  // pub sorted_modules: Vec<NormalModuleId>,
  pub metas: LinkingMetadataVec,
  pub symbol_db: SymbolRefDb,
  pub runtime: RuntimeModuleBrief,
  pub warnings: Vec<anyhow::Error>,
  pub errors: Vec<anyhow::Error>,
  pub used_symbol_refs: FxHashSet<SymbolRef>,
  pub dynamic_import_exports_usage_map: FxHashMap<ModuleIdx, DynamicImportExportsUsage>,
}

#[derive(Debug)]
pub struct LinkStage<'a> {
  pub module_table: ModuleTable,
  pub entries: Vec<EntryPoint>,
  pub symbols: SymbolRefDb,
  pub runtime: RuntimeModuleBrief,
  pub sorted_modules: Vec<ModuleIdx>,
  pub metas: LinkingMetadataVec,
  pub warnings: Vec<anyhow::Error>,
  pub errors: Vec<anyhow::Error>,
  pub ast_table: IndexEcmaAst,
  pub options: &'a SharedOptions,
  pub used_symbol_refs: FxHashSet<SymbolRef>,
  pub dynamic_import_exports_usage_map: FxHashMap<ModuleIdx, DynamicImportExportsUsage>,
}

impl<'a> LinkStage<'a> {
  pub fn new(scan_stage_output: ScanStageOutput, options: &'a SharedOptions) -> Self {
    todo!()
  }

  pub fn link(mut self) -> LinkStageOutput {
    todo!()
  }

  fn determine_module_exports_kind(&mut self) {
    todo!()
  }

  fn reference_needed_symbols(&mut self) {
    todo!()
  }

  fn create_exports_for_ecma_modules(&mut self) {
    todo!()
  }

  fn patch_module_dependencies(&mut self) {
    todo!()
  }
}
