use indexmap::IndexSet;
use minipack_common::{Module, ModuleIdx, NamespaceAlias, Specifier, SymbolRef, SymbolRefDb};
use minipack_utils::rstr::Rstr;
use oxc::span::CompactStr;
use rustc_hash::{FxHashMap, FxHashSet};

use crate::types::{IndexModules, LinkingMetadataVec, SharedOptions};

#[derive(Clone, Debug)]
struct ImportTracker {
  pub importer: ModuleIdx,
  pub importee: ModuleIdx,
  pub imported: Specifier,
  pub imported_as: SymbolRef,
}

#[derive(Debug, PartialEq, Eq)]
pub enum MatchImportKind {
  Cycle,
  NoMatch,
  Namespace(SymbolRef),
  Normal { symbol: SymbolRef, reexports: Vec<SymbolRef> },
  NormalAndNamespace { namespace_ref: SymbolRef, alias: Rstr },
}

#[derive(Debug)]
pub enum ImportStatus {
  NoMatch,
  Matched(SymbolRef),
  External(SymbolRef),
}

pub(super) struct BindImportsAndExportsContext<'a> {
  pub module_table: &'a IndexModules,
  pub metadata: &'a mut LinkingMetadataVec,
  pub symbol_db: &'a mut SymbolRefDb,
  pub options: &'a SharedOptions,
  pub errors: Vec<anyhow::Error>,
  pub side_effects_modules: &'a FxHashSet<ModuleIdx>,
  pub external_imports: FxHashMap<ModuleIdx, FxHashMap<CompactStr, IndexSet<SymbolRef>>>,
  pub normal_symbol_exports_chain_map: &'a mut FxHashMap<SymbolRef, Vec<SymbolRef>>,
}

impl BindImportsAndExportsContext<'_> {
  pub fn match_imports_with_exports(&mut self, module_idx: ModuleIdx) {
    let Module::Normal(module) = &self.module_table[module_idx] else {
      return;
    };

    let is_esm = self.options.format.is_esm();
    for (&imported_as_ref, named_import) in &module.named_imports {
      let import_record = &module.import_records[named_import.record_id];
      let is_external = matches!(self.module_table[import_record.state], Module::External(_));

      if is_esm && is_external {
        if let Specifier::Literal(ref name) = named_import.imported {
          self
            .external_imports
            .entry(import_record.state)
            .or_default()
            .entry(name.inner().clone())
            .or_default()
            .insert(imported_as_ref);
        }
      }

      let ret = self.match_import_with_export(
        self.module_table,
        &mut Vec::default(),
        ImportTracker {
          importer: module_idx,
          importee: import_record.state,
          imported: named_import.imported.clone(),
          imported_as: imported_as_ref,
        },
      );

      match ret {
        MatchImportKind::Namespace(namespace_ref) => {
          self.symbol_db.link(imported_as_ref, namespace_ref);
        }
        MatchImportKind::Normal { symbol, reexports } => {
          for r in &reexports {
            if self.side_effects_modules.contains(&r.owner) {
              self.metadata[module_idx].dependencies.insert(r.owner);
            }
          }
          self.normal_symbol_exports_chain_map.insert(imported_as_ref, reexports);
          self.symbol_db.link(imported_as_ref, symbol);
        }
        MatchImportKind::NormalAndNamespace { namespace_ref, alias } => {
          self.symbol_db.get_mut(imported_as_ref).namespace_alias =
            Some(NamespaceAlias { property_name: alias, namespace_ref });
        }
        MatchImportKind::NoMatch => {
          let importee = &self.module_table[import_record.state];
          self.errors.push(anyhow::anyhow!(
            r#""{}" is not exported by "{}", imported by "{}"."#,
            named_import.imported,
            importee.stable_id(),
            module.stable_id
          ));
        }
        MatchImportKind::Cycle => {}
      }
    }
  }

  fn match_import_with_export(
    &mut self,
    module_table: &IndexModules,
    tracker_stack: &mut Vec<ImportTracker>,
    mut tracker: ImportTracker,
  ) -> MatchImportKind {
    let mut reexports = vec![];
    loop {
      for prev_tracker in tracker_stack.iter().rev() {
        if prev_tracker.importer == tracker.importer
          && prev_tracker.imported_as == tracker.imported_as
        {
          return MatchImportKind::Cycle;
        }
      }

      tracker_stack.push(tracker.clone());

      break match self.advance_import_tracker(&tracker) {
        ImportStatus::NoMatch => MatchImportKind::NoMatch,
        ImportStatus::Matched(symbol) => {
          // If this is a re-export of another import, continue for another
          // iteration of the loop to resolve that import as well
          let owner = module_table[symbol.owner].as_normal().unwrap();
          if let Some(another_named_import) = owner.named_imports.get(&symbol) {
            let import_record = &owner.import_records[another_named_import.record_id];
            match &self.module_table[import_record.state] {
              Module::External(_) => MatchImportKind::Normal {
                symbol: another_named_import.imported_as,
                reexports: vec![],
              },
              Module::Normal(importee) => {
                tracker.importee = importee.idx;
                tracker.importer = owner.idx;
                tracker.imported = another_named_import.imported.clone();
                tracker.imported_as = another_named_import.imported_as;
                reexports.push(another_named_import.imported_as);
                continue;
              }
            }
          } else {
            MatchImportKind::Normal { symbol, reexports }
          }
        }
        ImportStatus::External(symbol_ref) => {
          if self.options.format.is_esm() {
            // Imports from external modules should not be converted to CommonJS
            // if the output format preserves the original ES6 import statements
            MatchImportKind::Normal { symbol: tracker.imported_as, reexports: vec![] }
          } else {
            match &tracker.imported {
              Specifier::Star => MatchImportKind::Namespace(symbol_ref),
              Specifier::Literal(alias) => MatchImportKind::NormalAndNamespace {
                namespace_ref: symbol_ref,
                alias: alias.clone(),
              },
            }
          }
        }
      };
    }
  }

  fn advance_import_tracker(&self, tracker: &ImportTracker) -> ImportStatus {
    let importer = self.module_table[tracker.importer].as_normal().unwrap();
    let named_import = &importer.named_imports[&tracker.imported_as];
    let importee_idx = importer.import_records[named_import.record_id].state;

    let importee = match &self.module_table[importee_idx] {
      Module::Normal(importee) => importee.as_ref(),
      Module::External(external) => return ImportStatus::External(external.namespace_ref),
    };

    match &named_import.imported {
      Specifier::Star => ImportStatus::Matched(importee.namespace_object_ref),
      Specifier::Literal(literal_imported) => {
        let resolved_exports = &self.metadata[importee_idx].resolved_exports;
        if let Some(symbol_ref) = resolved_exports.get(literal_imported) {
          ImportStatus::Matched(*symbol_ref)
        } else {
          ImportStatus::NoMatch
        }
      }
    }
  }
}
