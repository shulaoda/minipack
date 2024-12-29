mod determine_module_exports_kind;
mod generate_lazy_export;
mod reference_needed_symbols;
mod sort_modules;
mod wrap_modules;

use minipack_common::{
  dynamic_import_usage::DynamicImportExportsUsage, EntryPoint, ImportKind, ModuleIdx, ModuleTable,
  RuntimeModuleBrief, SymbolRef, SymbolRefDb,
};
use oxc_index::IndexVec;
use rustc_hash::{FxHashMap, FxHashSet};

use crate::types::{
  linking_metadata::{LinkingMetadata, LinkingMetadataVec},
  IndexEcmaAst, SharedOptions,
};

use super::scan::ScanStageOutput;

#[derive(Debug)]
pub struct LinkStageOutput {
  pub module_table: ModuleTable,
  pub entry_points: Vec<EntryPoint>,
  pub index_ecma_ast: IndexEcmaAst,
  pub sorted_modules: Vec<ModuleIdx>,
  pub metadata: LinkingMetadataVec,
  pub symbol_ref_db: SymbolRefDb,
  pub runtime_brief: RuntimeModuleBrief,
  pub warnings: Vec<anyhow::Error>,
  pub errors: Vec<anyhow::Error>,
  pub used_symbol_refs: FxHashSet<SymbolRef>,
  pub dyn_import_usage_map: FxHashMap<ModuleIdx, DynamicImportExportsUsage>,
}

#[derive(Debug)]
pub struct LinkStage<'a> {
  pub module_table: ModuleTable,
  pub entry_points: Vec<EntryPoint>,
  pub symbol_ref_db: SymbolRefDb,
  pub runtime_brief: RuntimeModuleBrief,
  pub sorted_modules: Vec<ModuleIdx>,
  pub metadata: LinkingMetadataVec,
  pub warnings: Vec<anyhow::Error>,
  pub errors: Vec<anyhow::Error>,
  pub index_ecma_ast: IndexEcmaAst,
  pub options: &'a SharedOptions,
  pub used_symbol_refs: FxHashSet<SymbolRef>,
  pub dyn_import_usage_map: FxHashMap<ModuleIdx, DynamicImportExportsUsage>,
}

impl<'a> LinkStage<'a> {
  pub fn new(scan_stage_output: ScanStageOutput, options: &'a SharedOptions) -> Self {
    let ScanStageOutput {
      module_table,
      index_ecma_ast,
      symbol_ref_db,
      entry_points,
      runtime_brief,
      warnings,
      dyn_import_usage_map,
    } = scan_stage_output;

    let metadata = module_table
      .modules
      .iter()
      .map(|module| {
        let dependencies = module
          .import_records()
          .iter()
          .filter_map(|rec| {
            (!matches!(rec.kind, ImportKind::DynamicImport) || options.inline_dynamic_imports)
              .then(|| rec.resolved_module)
          })
          .collect();

        let star_exports_from_external_modules =
          module.as_normal().map_or_else(Vec::new, |inner| {
            inner.star_exports_from_external_modules(&module_table.modules).collect()
          });

        LinkingMetadata {
          dependencies,
          star_exports_from_external_modules,
          ..LinkingMetadata::default()
        }
      })
      .collect::<IndexVec<ModuleIdx, _>>();

    Self {
      sorted_modules: Vec::new(),
      metadata,
      module_table,
      entry_points,
      symbol_ref_db,
      runtime_brief,
      warnings,
      errors: vec![],
      index_ecma_ast,
      dyn_import_usage_map,
      options,
      used_symbol_refs: FxHashSet::default(),
    }
  }

  pub fn link(mut self) -> LinkStageOutput {
    self.sort_modules();

    self.determine_module_exports_kind();
    self.wrap_modules();
    self.generate_lazy_export();

    self.reference_needed_symbols();

    LinkStageOutput {
      module_table: self.module_table,
      entry_points: self.entry_points,
      sorted_modules: self.sorted_modules,
      metadata: self.metadata,
      symbol_ref_db: self.symbol_ref_db,
      runtime_brief: self.runtime_brief,
      warnings: self.warnings,
      errors: self.errors,
      index_ecma_ast: self.index_ecma_ast,
      used_symbol_refs: self.used_symbol_refs,
      dyn_import_usage_map: self.dyn_import_usage_map,
    }
  }
}
