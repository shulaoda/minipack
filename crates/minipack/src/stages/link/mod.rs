mod sort_modules;

use std::ptr::addr_of;

use minipack_common::{
  dynamic_import_usage::DynamicImportExportsUsage, EntryPoint, ExportsKind, ImportKind, Module,
  ModuleIdx, ModuleTable, OutputFormat, RuntimeModuleBrief, SymbolRef, SymbolRefDb, WrapKind,
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

  fn determine_module_exports_kind(&mut self) {
    let entry_ids_set = self.entry_points.iter().map(|e| e.id).collect::<FxHashSet<_>>();
    self.module_table.modules.iter().filter_map(Module::as_normal).for_each(|importer| {
      // TODO(hyf0): should check if importer is a js module
      importer.import_records.iter().for_each(|rec| {
        let importee_id = rec.resolved_module;
        let Module::Normal(importee) = &self.module_table.modules[importee_id] else {
          return;
        };

        match rec.kind {
          ImportKind::Import => {
            if matches!(importee.exports_kind, ExportsKind::None)
              && !importee.meta.has_lazy_export()
            {
              // `import` a module that has `ExportsKind::None`, which will be turned into `ExportsKind::Esm`
              // SAFETY: If `importee` and `importer` are different, so this is safe. If they are the same, then behaviors are still expected.
              unsafe {
                let importee_mut = addr_of!(*importee).cast_mut();
                (*importee_mut).exports_kind = ExportsKind::Esm;
              }
            }
          }
          ImportKind::Require => match importee.exports_kind {
            ExportsKind::Esm => {
              self.metadata[importee.idx].wrap_kind = WrapKind::Esm;
            }
            ExportsKind::CommonJs => {
              self.metadata[importee.idx].wrap_kind = WrapKind::Cjs;
            }
            ExportsKind::None => {
              self.metadata[importee.idx].wrap_kind = WrapKind::Cjs;
              // SAFETY: If `importee` and `importer` are different, so this is safe. If they are the same, then behaviors are still expected.
              // A module with `ExportsKind::None` that `require` self should be turned into `ExportsKind::CommonJs`.
              unsafe {
                let importee_mut = addr_of!(*importee).cast_mut();
                (*importee_mut).exports_kind = ExportsKind::CommonJs;
              }
            }
          },
          ImportKind::DynamicImport => {
            if self.options.inline_dynamic_imports {
              // For iife, then import() is just a require() that
              // returns a promise, so the imported file must also be wrapped
              match importee.exports_kind {
                ExportsKind::Esm => {
                  self.metadata[importee.idx].wrap_kind = WrapKind::Esm;
                }
                ExportsKind::CommonJs => {
                  self.metadata[importee.idx].wrap_kind = WrapKind::Cjs;
                }
                ExportsKind::None => {
                  self.metadata[importee.idx].wrap_kind = WrapKind::Cjs;
                  // SAFETY: If `importee` and `importer` are different, so this is safe. If they are the same, then behaviors are still expected.
                  // A module with `ExportsKind::None` that `require` self should be turned into `ExportsKind::CommonJs`.
                  unsafe {
                    let importee_mut = addr_of!(*importee).cast_mut();
                    (*importee_mut).exports_kind = ExportsKind::CommonJs;
                  }
                }
              }
            }
          }
          ImportKind::AtImport => {
            unreachable!("A Js module would never import a CSS module via `@import`");
          }
          ImportKind::UrlImport => {
            unreachable!("A Js module would never import a CSS module via `url()`");
          }
          ImportKind::NewUrl => {}
        }
      });

      let is_entry = entry_ids_set.contains(&importer.idx);
      if matches!(importer.exports_kind, ExportsKind::CommonJs)
        && (!is_entry || matches!(self.options.format, OutputFormat::Esm))
      {
        self.metadata[importer.idx].wrap_kind = WrapKind::Cjs;
      }
    });
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
