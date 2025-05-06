mod bind_imports_and_exports;
mod create_exports_for_ecma_modules;
mod determine_side_effects;
mod include_statements;
mod patch_module_dependencies;
mod reference_needed_symbols;
mod sort_modules;

use minipack_common::{
  EntryPoint, EntryPointKind, ImportKind, ModuleIdx, RuntimeModuleBrief, SymbolRef, SymbolRefDb,
};
use oxc_index::IndexVec;
use rustc_hash::FxHashSet;

use crate::types::{
  IndexEcmaAst, IndexModules, LinkingMetadataVec, SharedOptions, linking_metadata::LinkingMetadata,
};

use super::scan_stage::ScanStageOutput;

#[derive(Debug)]
pub struct LinkStageOutput {
  pub module_table: IndexModules,
  pub entry_points: Vec<EntryPoint>,
  pub index_ecma_ast: IndexEcmaAst,
  pub metadata: LinkingMetadataVec,
  pub symbols: SymbolRefDb,
  pub runtime_module: RuntimeModuleBrief,
  pub warnings: Vec<anyhow::Error>,
  pub errors: Vec<anyhow::Error>,
  pub used_symbol_refs: FxHashSet<SymbolRef>,
  pub lived_entry_points: FxHashSet<ModuleIdx>,
}

#[derive(Debug)]
pub struct LinkStage<'a> {
  pub module_table: IndexModules,
  pub entry_points: Vec<EntryPoint>,
  pub symbols: SymbolRefDb,
  pub runtime_module: RuntimeModuleBrief,
  pub sorted_modules: Vec<ModuleIdx>,
  pub metadata: LinkingMetadataVec,
  pub warnings: Vec<anyhow::Error>,
  pub errors: Vec<anyhow::Error>,
  pub index_ecma_ast: IndexEcmaAst,
  pub options: &'a SharedOptions,
  pub used_symbol_refs: FxHashSet<SymbolRef>,
}

impl<'a> LinkStage<'a> {
  pub fn new(scan_stage_output: ScanStageOutput, options: &'a SharedOptions) -> Self {
    let ScanStageOutput {
      module_table,
      index_ecma_ast,
      symbols,
      entry_points,
      runtime_module,
      warnings,
    } = scan_stage_output;

    let metadata = module_table
      .iter()
      .map(|module| {
        let dependencies = module
          .import_records()
          .iter()
          .filter_map(|rec| match rec.kind {
            ImportKind::DynamicImport => None,
            _ => Some(rec.state),
          })
          .collect();

        let star_exports_from_external_modules =
          module.as_normal().map_or_else(Vec::new, |inner| {
            inner.star_exports_from_external_modules(&module_table).collect()
          });

        LinkingMetadata {
          dependencies,
          star_exports_from_external_modules,
          ..LinkingMetadata::default()
        }
      })
      .collect::<IndexVec<ModuleIdx, _>>();

    Self {
      metadata,
      module_table,
      entry_points,
      symbols,
      runtime_module,
      warnings,
      errors: vec![],
      sorted_modules: vec![],
      index_ecma_ast,
      options,
      used_symbol_refs: FxHashSet::default(),
    }
  }

  pub fn link(mut self) -> LinkStageOutput {
    self.sort_modules();
    self.determine_side_effects();
    self.bind_imports_and_exports();
    self.create_exports_for_ecma_modules();
    self.reference_needed_symbols();
    self.include_statements();
    self.patch_module_dependencies();

    LinkStageOutput {
      lived_entry_points: self.get_lived_entry(),
      module_table: self.module_table,
      entry_points: self.entry_points,
      metadata: self.metadata,
      symbols: self.symbols,
      runtime_module: self.runtime_module,
      warnings: self.warnings,
      errors: self.errors,
      index_ecma_ast: self.index_ecma_ast,
      used_symbol_refs: self.used_symbol_refs,
    }
  }

  #[inline]
  fn get_lived_entry(&self) -> FxHashSet<ModuleIdx> {
    self
      .entry_points
      .iter()
      .filter_map(|item| match item.kind {
        EntryPointKind::UserDefined => Some(item.id),
        EntryPointKind::DynamicImport => {
          // At least one statement that create this entry is included
          let lived = item.related_stmt_infos.iter().any(|(module_idx, stmt_idx)| {
            let module =
              &self.module_table[*module_idx].as_normal().expect("should be a normal module");
            let stmt_info = &module.stmt_infos[*stmt_idx];
            stmt_info.is_included
          });
          lived.then_some(item.id)
        }
      })
      .collect::<FxHashSet<ModuleIdx>>()
  }
}
