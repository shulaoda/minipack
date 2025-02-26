mod bind_imports_and_exports;
mod create_exports_for_ecma_modules;
mod determine_side_effects;
mod generate_lazy_export;
mod include_statements;
mod patch_module_dependencies;
mod reference_needed_symbols;
mod sort_modules;
mod wrap_modules;

use minipack_common::{
  EntryPoint, EntryPointKind, ImportKind, Module, ModuleIdx, RuntimeModuleBrief, SymbolRef,
  SymbolRefDb, dynamic_import_usage::DynamicImportExportsUsage,
};
use oxc_index::IndexVec;
use rustc_hash::{FxHashMap, FxHashSet};

use crate::types::{
  IndexAstScope, IndexEcmaAst, IndexModules, SharedOptions,
  linking_metadata::{LinkingMetadata, LinkingMetadataVec},
};

use super::scan_stage::ScanStageOutput;

#[derive(Debug)]
pub struct LinkStageOutput {
  pub modules: IndexModules,
  pub entry_points: Vec<EntryPoint>,
  pub index_ecma_ast: IndexEcmaAst,
  pub metadata: LinkingMetadataVec,
  pub symbols: SymbolRefDb,
  pub runtime_module: RuntimeModuleBrief,
  pub warnings: Vec<anyhow::Error>,
  pub errors: Vec<anyhow::Error>,
  pub ast_scope_table: IndexAstScope,
  pub used_symbol_refs: FxHashSet<SymbolRef>,
  pub dyn_import_usage_map: FxHashMap<ModuleIdx, DynamicImportExportsUsage>,
  pub lived_entry_points: FxHashSet<ModuleIdx>,
}

#[derive(Debug)]
pub struct LinkStage<'a> {
  pub modules: IndexModules,
  pub entry_points: Vec<EntryPoint>,
  pub symbols: SymbolRefDb,
  pub runtime_module: RuntimeModuleBrief,
  pub sorted_modules: Vec<ModuleIdx>,
  pub metadata: LinkingMetadataVec,
  pub warnings: Vec<anyhow::Error>,
  pub errors: Vec<anyhow::Error>,
  pub index_ecma_ast: IndexEcmaAst,
  pub ast_scope_table: IndexAstScope,
  pub options: &'a SharedOptions,
  pub used_symbol_refs: FxHashSet<SymbolRef>,
  pub dyn_import_usage_map: FxHashMap<ModuleIdx, DynamicImportExportsUsage>,
}

impl<'a> LinkStage<'a> {
  pub fn new(scan_stage_output: ScanStageOutput, options: &'a SharedOptions) -> Self {
    let ScanStageOutput {
      modules,
      index_ecma_ast,
      index_ast_scope,
      symbols,
      entry_points,
      runtime_module,
      warnings,
      dyn_import_usage_map,
    } = scan_stage_output;

    let metadata = modules
      .iter()
      .map(|module| {
        let dependencies = module
          .import_records()
          .iter()
          .filter_map(|rec| match rec.kind {
            ImportKind::DynamicImport => None,
            _ => Some(rec.resolved_module),
          })
          .collect();

        let star_exports_from_external_modules =
          module.as_normal().map_or_else(Vec::new, |inner| {
            inner.star_exports_from_external_modules(&modules).collect()
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
      modules,
      entry_points,
      symbols,
      runtime_module,
      warnings,
      errors: vec![],
      sorted_modules: vec![],
      index_ecma_ast,
      dyn_import_usage_map,
      options,
      ast_scope_table: index_ast_scope,
      used_symbol_refs: FxHashSet::default(),
    }
  }

  pub fn link(mut self) -> LinkStageOutput {
    self.sort_modules();
    self.compute_tla();
    self.wrap_modules();
    self.generate_lazy_export();
    self.determine_side_effects();
    self.bind_imports_and_exports();
    self.create_exports_for_ecma_modules();
    self.reference_needed_symbols();
    self.include_statements();
    self.patch_module_dependencies();

    LinkStageOutput {
      lived_entry_points: self.get_lived_entry(),
      modules: self.modules,
      entry_points: self.entry_points,
      metadata: self.metadata,
      symbols: self.symbols,
      runtime_module: self.runtime_module,
      warnings: self.warnings,
      errors: self.errors,
      index_ecma_ast: self.index_ecma_ast,
      used_symbol_refs: self.used_symbol_refs,
      dyn_import_usage_map: self.dyn_import_usage_map,
      ast_scope_table: self.ast_scope_table,
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
          let lived = item
            .related_stmt_infos
            .iter()
            .filter(|(module_idx, stmt_idx)| {
              let module =
                &self.modules[*module_idx].as_normal().expect("should be a normal module");
              let stmt_info = &module.stmt_infos[*stmt_idx];
              stmt_info.is_included
            })
            .count()
            > 0;
          lived.then_some(item.id)
        }
      })
      .collect::<FxHashSet<ModuleIdx>>()
  }

  fn compute_tla(&mut self) {
    fn is_tla(
      module_idx: ModuleIdx,
      module_table: &IndexVec<ModuleIdx, Module>,
      // `None` means the module is in visiting
      visited_map: &mut FxHashMap<ModuleIdx, Option<bool>>,
    ) -> bool {
      if let Some(memorized) = visited_map.get(&module_idx) {
        memorized.unwrap_or(false)
      } else {
        visited_map.insert(module_idx, None);
        let module = &module_table[module_idx];
        let is_self_tla = module.as_normal().is_some_and(|module| module.has_top_level_await);
        if is_self_tla {
          // If the module itself contains top-level await, then it is TLA.
          visited_map.insert(module_idx, Some(true));
          return true;
        }

        let contains_tla_dependency = module
          .import_records()
          .iter()
          // TODO: require TLA module should give a error
          .filter(|rec| matches!(rec.kind, ImportKind::Import))
          .any(|rec| {
            let importee = &module_table[rec.resolved_module];
            is_tla(importee.idx(), module_table, visited_map)
          });

        visited_map.insert(module_idx, Some(contains_tla_dependency));
        contains_tla_dependency
      }
    }

    let mut visited_map = FxHashMap::default();

    self.modules.iter().filter_map(|m| m.as_normal()).for_each(|module| {
      self.metadata[module.idx].is_tla_or_contains_tla_dependency =
        is_tla(module.idx, &self.modules, &mut visited_map);
    });
  }
}
