use std::borrow::Cow;

use arcstr::ArcStr;
use indexmap::IndexSet;
use minipack_common::{
  Module, ModuleIdx, NamespaceAlias, NormalModule, OutputFormat, ResolvedExport, Specifier,
  SymbolOrMemberExprRef, SymbolRef, SymbolRefDb,
};
use minipack_utils::{
  ecmascript::is_validate_identifier_name,
  rayon::{IntoParallelRefIterator, IntoParallelRefMutIterator, ParallelIterator},
  rstr::{Rstr, ToRstr},
};
use oxc::span::CompactStr;

use rustc_hash::{FxHashMap, FxHashSet};

use crate::{
  types::{IndexModules, LinkingMetadataVec, SharedOptions},
  utils::ecmascript::legitimize_identifier_name,
};

use super::LinkStage;

#[derive(Clone, Debug)]
struct ImportTracker {
  pub importer: ModuleIdx,
  pub importee: ModuleIdx,
  pub imported: Specifier,
  pub imported_as: SymbolRef,
}

pub struct MatchingContext {
  tracker_stack: Vec<ImportTracker>,
}

impl MatchingContext {
  fn current_tracker(&self) -> &ImportTracker {
    self.tracker_stack.last().expect("tracker_stack is not empty")
  }
}

#[derive(Debug, Eq)]
pub struct MatchImportKindNormal {
  symbol: SymbolRef,
  reexports: Vec<SymbolRef>,
}

impl PartialEq for MatchImportKindNormal {
  fn eq(&self, other: &Self) -> bool {
    self.symbol == other.symbol
  }
}

#[derive(Debug, PartialEq, Eq)]
pub enum MatchImportKind {
  /// The import is either external or not defined.
  _Ignore,
  // "sourceIndex" and "ref" are in use
  Normal(MatchImportKindNormal),
  // "namespaceRef" and "alias" are in use
  Namespace {
    namespace_ref: SymbolRef,
  },
  // Both "matchImportNormal" and "matchImportNamespace"
  NormalAndNamespace {
    namespace_ref: SymbolRef,
    alias: Rstr,
  },
  // The import could not be evaluated due to a cycle
  Cycle,
  // The import resolved to multiple symbols via "export * from"
  Ambiguous {
    symbol_ref: SymbolRef,
    potentially_ambiguous_symbol_refs: Vec<SymbolRef>,
  },
  NoMatch,
}

#[derive(Debug)]
pub enum ImportStatus {
  /// The imported file has no matching export
  NoMatch {
    // importee_id: NormalModuleId,
  },

  /// The imported file has a matching export
  Found {
    // owner: NormalModuleId,
    symbol: SymbolRef,
    potentially_ambiguous_export_star_refs: Vec<SymbolRef>,
  },

  /// The imported file was disabled by mapping it to false in the "browser" field of package.json
  _Disabled,

  /// The imported file is external and has unknown exports
  External(SymbolRef),
}

impl LinkStage<'_> {
  /// Notices:
  /// - For external import like
  /// ```js
  /// // main.js
  /// import { a } from 'external';
  ///
  /// // foo.js
  /// import { a } from 'external';
  /// export { a }
  /// ```
  ///
  /// Unlike import from normal modules, the imported variable deosn't have a place that declared the variable. So we consider `import { a } from 'external'` in `foo.js` as the declaration statement of `a`.
  pub fn bind_imports_and_exports(&mut self) {
    // Initialize `resolved_exports` to prepare for matching imports with exports
    self.metadata.iter_mut_enumerated().for_each(|(module_id, meta)| {
      let Module::Normal(module) = &self.module_table[module_id] else {
        return;
      };
      let mut resolved_exports = module
        .named_exports
        .iter()
        .map(|(name, local)| {
          let resolved_export = ResolvedExport {
            symbol_ref: local.referenced,
            potentially_ambiguous_symbol_refs: None,
          };
          (name.clone(), resolved_export)
        })
        .collect::<FxHashMap<_, _>>();

      let mut module_stack = vec![];
      if module.has_star_export() {
        Self::add_exports_for_export_star(
          &self.module_table,
          &mut resolved_exports,
          module_id,
          &mut module_stack,
        );
      }
      meta.resolved_exports = resolved_exports;
    });
    let side_effects_modules = self
      .module_table
      .iter_enumerated()
      .filter_map(|(idx, item)| item.side_effects().has_side_effects().then_some(idx))
      .collect::<FxHashSet<ModuleIdx>>();
    let mut normal_symbol_exports_chain_map = FxHashMap::default();
    let mut binding_ctx = BindImportsAndExportsContext {
      module_table: &self.module_table,
      metas: &mut self.metadata,
      symbol_db: &mut self.symbols,
      options: self.options,
      errors: Vec::default(),
      warnings: Vec::default(),
      external_import_binding_merger: FxHashMap::default(),
      side_effects_modules: &side_effects_modules,
      normal_symbol_exports_chain_map: &mut normal_symbol_exports_chain_map,
    };
    self.module_table.iter().for_each(|module| {
      binding_ctx.match_imports_with_exports(module.idx());
    });

    self.errors.extend(binding_ctx.errors);
    self.warnings.extend(binding_ctx.warnings);

    for (module_idx, map) in &binding_ctx.external_import_binding_merger {
      for (key, symbol_set) in map {
        let name = if key.as_str() == "default" {
          let key = symbol_set
            .first()
            .map_or_else(|| key.clone(), |sym_ref| sym_ref.name(&self.symbols).into());
          Cow::Owned(key)
        } else if is_validate_identifier_name(key.as_str()) {
          Cow::Borrowed(key)
        } else {
          let legal_name = legitimize_identifier_name(key);
          Cow::Owned(legal_name.as_ref().into())
        };
        let target_symbol = self.symbols.create_facade_root_symbol_ref(*module_idx, &name);
        for symbol_ref in symbol_set {
          self.symbols.link(*symbol_ref, target_symbol);
        }
      }
    }
    self.metadata.par_iter_mut().for_each(|meta| {
      let mut sorted_and_non_ambiguous_resolved_exports = vec![];
      'next_export: for (exported_name, resolved_export) in &meta.resolved_exports {
        if let Some(potentially_ambiguous_symbol_refs) =
          &resolved_export.potentially_ambiguous_symbol_refs
        {
          let main_ref = self.symbols.canonical_ref_for(resolved_export.symbol_ref);

          for ambiguous_ref in potentially_ambiguous_symbol_refs {
            let ambiguous_ref = self.symbols.canonical_ref_for(*ambiguous_ref);
            if main_ref != ambiguous_ref {
              continue 'next_export;
            }
          }
        };
        sorted_and_non_ambiguous_resolved_exports.push(exported_name.clone());
      }
      sorted_and_non_ambiguous_resolved_exports.sort_unstable();
      meta.sorted_and_non_ambiguous_resolved_exports = sorted_and_non_ambiguous_resolved_exports;
    });
    self.resolve_member_expr_refs(&side_effects_modules, &normal_symbol_exports_chain_map);
  }

  fn add_exports_for_export_star(
    normal_modules: &IndexModules,
    resolve_exports: &mut FxHashMap<Rstr, ResolvedExport>,
    module_id: ModuleIdx,
    module_stack: &mut Vec<ModuleIdx>,
  ) {
    if module_stack.contains(&module_id) {
      return;
    }

    module_stack.push(module_id);

    let Module::Normal(module) = &normal_modules[module_id] else {
      return;
    };

    for dep_id in module.star_export_module_ids() {
      let Module::Normal(dep_module) = &normal_modules[dep_id] else {
        continue;
      };

      for (exported_name, named_export) in &dep_module.named_exports {
        // ES6 export star statements ignore exports named "default"
        if exported_name.as_str() == "default" {
          continue;
        }
        // This export star is shadowed if any file in the stack has a matching real named export
        if module_stack
          .iter()
          .filter_map(|id| normal_modules[*id].as_normal())
          .any(|module| module.named_exports.contains_key(exported_name))
        {
          continue;
        }

        // We have filled `resolve_exports` with `named_exports`. If the export is already exists, it means that the importer
        // has a named export with the same name. So the export from dep module is shadowed.
        if let Some(resolved_export) = resolve_exports.get_mut(exported_name) {
          if named_export.referenced != resolved_export.symbol_ref {
            resolved_export
              .potentially_ambiguous_symbol_refs
              .get_or_insert(Vec::default())
              .push(named_export.referenced);
          }
        } else {
          let resolved_export = ResolvedExport {
            symbol_ref: named_export.referenced,
            potentially_ambiguous_symbol_refs: None,
          };
          resolve_exports.insert(exported_name.clone(), resolved_export);
        }
      }

      Self::add_exports_for_export_star(normal_modules, resolve_exports, dep_id, module_stack);
    }

    module_stack.pop();
  }

  /// Try to find the final pointed `SymbolRef` of the member expression.
  /// ```js
  /// // index.js
  /// import * as foo_ns from './foo';
  /// foo_ns.bar_ns.c;
  /// // foo.js
  /// export * as bar_ns from './bar';
  /// // bar.js
  /// export const c = 1;
  /// ```
  /// The final pointed `SymbolRef` of `foo_ns.bar_ns.c` is the `c` in `bar.js`.
  fn resolve_member_expr_refs(
    &mut self,
    side_effects_modules: &FxHashSet<ModuleIdx>,
    normal_symbol_exports_chain_map: &FxHashMap<SymbolRef, Vec<SymbolRef>>,
  ) {
    let warnings = append_only_vec::AppendOnlyVec::new();
    let resolved_meta_data = self
      .module_table
      .par_iter()
      .map(|module| match module {
        Module::Normal(module) => {
          let mut resolved_map = FxHashMap::default();
          let mut side_effects_dependency = vec![];
          module.stmt_infos.iter().for_each(|stmt_info| {
            stmt_info.referenced_symbols.iter().for_each(|symbol_ref| {
              if let SymbolOrMemberExprRef::MemberExpr(member_expr_ref) = symbol_ref {
                // First get the canonical ref of `foo_ns`, then we get the `NormalModule#namespace_object_ref` of `foo.js`.
                let mut canonical_ref = self.symbols.canonical_ref_for(member_expr_ref.object_ref);
                let mut canonical_ref_owner: &NormalModule =
                  match &self.module_table[canonical_ref.owner] {
                    Module::Normal(module) => module,
                    Module::External(_) => return,
                  };
                let mut is_namespace_ref =
                  canonical_ref_owner.namespace_object_ref == canonical_ref;
                let mut ns_symbol_list = vec![];
                let mut cursor = 0;
                while cursor < member_expr_ref.props.len() && is_namespace_ref {
                  let name = &member_expr_ref.props[cursor];
                  let meta = &self.metadata[canonical_ref_owner.idx];
                  let export_symbol = meta.resolved_exports.get(&name.to_rstr());
                  let Some(export_symbol) = export_symbol else {
                    // when we try to resolve `a.b.c`, and found that `b` is not exported by module
                    // that `a` pointed to, convert the `a.b.c` into `void 0` if module `a` do not
                    // have any dynamic exports.
                    resolved_map.insert(
                      member_expr_ref.span,
                      (None, member_expr_ref.props[cursor..].to_vec()),
                    );
                    warnings.push(anyhow::anyhow!("Import `{}` will always be undefined because there is no matching export in '{}'", 
                    ArcStr::from(name.as_str()),
                    canonical_ref_owner.stable_id.to_string()));
                    break;
                  };
                  if !meta.sorted_and_non_ambiguous_resolved_exports.contains(&name.to_rstr()) {
                    resolved_map.insert(
                      member_expr_ref.span,
                      (None, member_expr_ref.props[cursor..].to_vec()),
                    );
                    return;
                  };

                  if let Some(chains) =
                    normal_symbol_exports_chain_map.get(&export_symbol.symbol_ref)
                  {
                    for item in chains {
                      if side_effects_modules.contains(&item.owner) {
                        side_effects_dependency.push(item.owner);
                      }
                    }
                  }
                  ns_symbol_list.push((canonical_ref, name.to_rstr()));
                  canonical_ref = self.symbols.canonical_ref_for(export_symbol.symbol_ref);
                  canonical_ref_owner =
                    self.module_table[canonical_ref.owner].as_normal().unwrap();
                  cursor += 1;
                  is_namespace_ref = canonical_ref_owner.namespace_object_ref == canonical_ref;
                }
                if cursor > 0 {
                  resolved_map.insert(
                    member_expr_ref.span,
                    (Some(canonical_ref), member_expr_ref.props[cursor..].to_vec()),
                  );
                }
              }
            });
          });

          (resolved_map, side_effects_dependency)
        }
        Module::External(_) => (FxHashMap::default(), vec![]),
      })
      .collect::<Vec<_>>();

    debug_assert_eq!(self.metadata.len(), resolved_meta_data.len());
    self.warnings.extend(warnings);
    self.metadata.iter_mut_enumerated().zip(resolved_meta_data).for_each(
      |((_idx, meta), (resolved_map, side_effects_dependency))| {
        meta.resolved_member_expr_refs = resolved_map;
        meta.dependencies.extend(side_effects_dependency);
      },
    );
  }
}

struct BindImportsAndExportsContext<'a> {
  pub module_table: &'a IndexModules,
  pub metas: &'a mut LinkingMetadataVec,
  pub symbol_db: &'a mut SymbolRefDb,
  pub options: &'a SharedOptions,
  pub errors: Vec<anyhow::Error>,
  pub warnings: Vec<anyhow::Error>,
  pub external_import_binding_merger:
    FxHashMap<ModuleIdx, FxHashMap<CompactStr, IndexSet<SymbolRef>>>,
  pub side_effects_modules: &'a FxHashSet<ModuleIdx>,
  pub normal_symbol_exports_chain_map: &'a mut FxHashMap<SymbolRef, Vec<SymbolRef>>,
}

impl BindImportsAndExportsContext<'_> {
  fn match_imports_with_exports(&mut self, module_id: ModuleIdx) {
    let Module::Normal(module) = &self.module_table[module_id] else {
      return;
    };
    let is_esm = matches!(self.options.format, OutputFormat::Esm);
    for (imported_as_ref, named_import) in &module.named_imports {
      let rec = &module.import_records[named_import.record_id];
      let is_external = matches!(self.module_table[rec.state], Module::External(_));
      if is_esm && is_external {
        if let Specifier::Literal(ref name) = named_import.imported {
          self
            .external_import_binding_merger
            .entry(rec.state)
            .or_default()
            .entry(name.inner().clone())
            .or_default()
            .insert(*imported_as_ref);
        }
      }
      let ret = self.match_import_with_export(
        self.module_table,
        &mut MatchingContext { tracker_stack: Vec::default() },
        ImportTracker {
          importer: module_id,
          importee: rec.state,
          imported: named_import.imported.clone(),
          imported_as: *imported_as_ref,
        },
      );
      match ret {
        MatchImportKind::Ambiguous { symbol_ref, potentially_ambiguous_symbol_refs } => {
          let importee = self.module_table[rec.state].stable_id().to_string();

          let mut exporter = Vec::with_capacity(potentially_ambiguous_symbol_refs.len() + 1);
          if let Some(owner) = self.module_table[symbol_ref.owner].as_normal() {
            if let Specifier::Literal(_) = &named_import.imported {
              exporter.push(owner.stable_id.to_string());
            }
          }

          exporter.extend(potentially_ambiguous_symbol_refs.iter().filter_map(|&symbol_ref| {
            let normal_module = self.module_table[symbol_ref.owner].as_normal()?;
            if let Specifier::Literal(_) = &named_import.imported {
              return Some(normal_module.stable_id.to_string());
            }
            None
          }));

          self.errors.push(anyhow::anyhow!(
            r#""{}" re-exports "{}" from one of the modules {} and {} (will be ignored)."#,
            importee,
            named_import.imported.to_string(),
            exporter.join(", "),
            exporter.iter().next_back().unwrap()
          ));
        }
        MatchImportKind::Normal(MatchImportKindNormal { symbol, reexports }) => {
          for r in &reexports {
            if self.side_effects_modules.contains(&r.owner) {
              self.metas[module_id].dependencies.insert(r.owner);
            }
          }
          self.normal_symbol_exports_chain_map.insert(*imported_as_ref, reexports);

          self.symbol_db.link(*imported_as_ref, symbol);
        }
        MatchImportKind::Namespace { namespace_ref } => {
          self.symbol_db.link(*imported_as_ref, namespace_ref);
        }
        MatchImportKind::NormalAndNamespace { namespace_ref, alias } => {
          self.symbol_db.get_mut(*imported_as_ref).namespace_alias =
            Some(NamespaceAlias { property_name: alias, namespace_ref });
        }
        MatchImportKind::NoMatch => {
          let importee = &self.module_table[rec.state];
          self.errors.push(anyhow::anyhow!(
            r#""{}" is not exported by "{}", imported by "{}"."#,
            named_import.imported,
            importee.stable_id(),
            module.stable_id
          ));
        }
        MatchImportKind::_Ignore | MatchImportKind::Cycle => {}
      }
    }
  }

  fn advance_import_tracker(&self, ctx: &mut MatchingContext) -> ImportStatus {
    let tracker = ctx.current_tracker();
    let importer =
      &self.module_table[tracker.importer].as_normal().expect("only normal module can be importer");
    let named_import = &importer.named_imports[&tracker.imported_as];

    // Is this an external file?
    let importee_id = importer.import_records[named_import.record_id].state;
    let importee = match &self.module_table[importee_id] {
      Module::Normal(importee) => importee.as_ref(),
      Module::External(external) => return ImportStatus::External(external.namespace_ref),
    };

    match &named_import.imported {
      Specifier::Star => ImportStatus::Found {
        symbol: importee.namespace_object_ref,
        // owner: importee_id,
        potentially_ambiguous_export_star_refs: vec![],
      },
      Specifier::Literal(literal_imported) => {
        if let Some(export) = self.metas[importee_id].resolved_exports.get(literal_imported) {
          ImportStatus::Found {
            // owner: importee_id,
            symbol: export.symbol_ref,
            potentially_ambiguous_export_star_refs: export
              .potentially_ambiguous_symbol_refs
              .clone()
              .unwrap_or_default(),
          }
        } else {
          ImportStatus::NoMatch {}
        }
      }
    }
  }

  fn match_import_with_export(
    &mut self,
    index_modules: &IndexModules,
    ctx: &mut MatchingContext,
    mut tracker: ImportTracker,
  ) -> MatchImportKind {
    let mut ambiguous_results = vec![];
    let mut reexports = vec![];
    let ret = loop {
      for prev_tracker in ctx.tracker_stack.iter().rev() {
        if prev_tracker.importer == tracker.importer
          && prev_tracker.imported_as == tracker.imported_as
        {
          // Cycle import. No need to continue, just return
          return MatchImportKind::Cycle;
        }
      }

      ctx.tracker_stack.push(tracker.clone());

      let import_status = self.advance_import_tracker(ctx);
      let kind = match import_status {
        ImportStatus::NoMatch { .. } => {
          break MatchImportKind::NoMatch;
        }
        ImportStatus::Found { symbol, potentially_ambiguous_export_star_refs, .. } => {
          for ambiguous_ref in &potentially_ambiguous_export_star_refs {
            let ambiguous_ref_owner = &index_modules[ambiguous_ref.owner];
            if let Some(another_named_import) =
              ambiguous_ref_owner.as_normal().unwrap().named_imports.get(ambiguous_ref)
            {
              let rec = &ambiguous_ref_owner.as_normal().unwrap().import_records
                [another_named_import.record_id];
              let ambiguous_result = self.match_import_with_export(
                index_modules,
                &mut MatchingContext { tracker_stack: ctx.tracker_stack.clone() },
                ImportTracker {
                  importer: ambiguous_ref_owner.idx(),
                  importee: rec.state,
                  imported: another_named_import.imported.clone(),
                  imported_as: another_named_import.imported_as,
                },
              );
              ambiguous_results.push(ambiguous_result);
            } else {
              ambiguous_results.push(MatchImportKind::Normal(MatchImportKindNormal {
                symbol: *ambiguous_ref,
                reexports: vec![],
              }));
            }
          }

          // If this is a re-export of another import, continue for another
          // iteration of the loop to resolve that import as well
          let owner = &index_modules[symbol.owner];
          if let Some(another_named_import) = owner.as_normal().unwrap().named_imports.get(&symbol)
          {
            let rec = &owner.as_normal().unwrap().import_records[another_named_import.record_id];
            match &self.module_table[rec.state] {
              Module::External(_) => {
                break MatchImportKind::Normal(MatchImportKindNormal {
                  symbol: another_named_import.imported_as,
                  reexports: vec![],
                });
              }
              Module::Normal(importee) => {
                tracker.importee = importee.idx;
                tracker.importer = owner.idx();
                tracker.imported = another_named_import.imported.clone();
                tracker.imported_as = another_named_import.imported_as;
                reexports.push(another_named_import.imported_as);
                continue;
              }
            }
          }

          break MatchImportKind::Normal(MatchImportKindNormal { symbol, reexports });
        }
        ImportStatus::_Disabled => todo!(),
        ImportStatus::External(symbol_ref) => {
          if self.options.format.is_esm() {
            // Imports from external modules should not be converted to CommonJS
            // if the output format preserves the original ES6 import statements
            break MatchImportKind::Normal(MatchImportKindNormal {
              symbol: tracker.imported_as,
              reexports: vec![],
            });
          }

          match &tracker.imported {
            Specifier::Star => MatchImportKind::Namespace { namespace_ref: symbol_ref },
            Specifier::Literal(alias) => MatchImportKind::NormalAndNamespace {
              namespace_ref: symbol_ref,
              alias: alias.clone(),
            },
          }
        }
      };
      break kind;
    };

    for ambiguous_result in &ambiguous_results {
      if *ambiguous_result != ret {
        if let MatchImportKind::Normal(MatchImportKindNormal { symbol, .. }) = ret {
          return MatchImportKind::Ambiguous {
            symbol_ref: symbol,
            potentially_ambiguous_symbol_refs: ambiguous_results
              .iter()
              .filter_map(|kind| match *kind {
                MatchImportKind::Normal(MatchImportKindNormal { symbol, .. }) => Some(symbol),
                MatchImportKind::Namespace { namespace_ref }
                | MatchImportKind::NormalAndNamespace { namespace_ref, .. } => Some(namespace_ref),
                _ => None,
              })
              .collect(),
          };
        }

        unreachable!("symbol should always exist");
      }
    }

    ret
  }
}
