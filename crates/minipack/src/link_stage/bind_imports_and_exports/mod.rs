mod bind_imports_and_exports_context;

use std::borrow::Cow;

use minipack_common::{Module, ModuleIdx, NormalModule, SymbolOrMemberExprRef, SymbolRef};
use minipack_utils::{
  ecmascript::is_validate_identifier_name,
  rayon::{IntoParallelRefIterator, IntoParallelRefMutIterator, ParallelIterator},
  rstr::{Rstr, ToRstr},
};
use rustc_hash::{FxHashMap, FxHashSet};

use crate::{types::IndexModules, utils::ecmascript::legitimize_identifier_name};

impl super::LinkStage {
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
  /// Unlike import from normal modules, the imported variable deosn't have a place that declared the variable.
  /// So we consider `import { a } from 'external'` in `foo.js` as the declaration statement of `a`.
  pub fn bind_imports_and_exports(&mut self) {
    // Initialize `resolved_exports` to prepare for matching imports with exports
    self.metadata.iter_mut_enumerated().for_each(|(module_idx, meta)| {
      let Module::Normal(module) = &self.module_table[module_idx] else {
        return;
      };

      let mut resolved_exports = module
        .named_exports
        .iter()
        .map(|(name, local)| (name.clone(), local.referenced))
        .collect::<FxHashMap<_, _>>();

      if module.has_star_export() {
        Self::add_exports_for_export_star(
          module_idx,
          &self.module_table,
          &mut vec![],
          &mut resolved_exports,
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

    let mut ctx = bind_imports_and_exports_context::BindImportsAndExportsContext {
      module_table: &self.module_table,
      metadata: &mut self.metadata,
      symbol_db: &mut self.symbol_ref_db,
      options: &self.options,
      errors: Vec::default(),
      external_imports: FxHashMap::default(),
      side_effects_modules: &side_effects_modules,
      normal_symbol_exports_chain_map: &mut normal_symbol_exports_chain_map,
    };

    self.module_table.iter().for_each(|module| {
      ctx.match_imports_with_exports(module.idx());
    });

    self.errors.extend(ctx.errors);

    for (module_idx, map) in &ctx.external_imports {
      for (key, symbol_set) in map {
        let name = if key.as_str() == "default" {
          let key = symbol_set
            .first()
            .map_or_else(|| key.clone(), |sym_ref| sym_ref.name(&self.symbol_ref_db).into());
          Cow::Owned(key)
        } else if is_validate_identifier_name(key.as_str()) {
          Cow::Borrowed(key)
        } else {
          let legal_name = legitimize_identifier_name(key);
          Cow::Owned(legal_name.as_ref().into())
        };
        let target_symbol = self.symbol_ref_db.create_facade_root_symbol_ref(*module_idx, &name);
        for symbol_ref in symbol_set {
          self.symbol_ref_db.link(*symbol_ref, target_symbol);
        }
      }
    }

    self.metadata.par_iter_mut().for_each(|meta| {
      let mut sorted_resolved_exports = meta.resolved_exports.keys().cloned().collect::<Vec<_>>();

      sorted_resolved_exports.sort_unstable();
      meta.sorted_resolved_exports = sorted_resolved_exports;
    });

    self.resolve_member_expr_refs(&side_effects_modules, &normal_symbol_exports_chain_map);
  }

  fn add_exports_for_export_star(
    module_idx: ModuleIdx,
    module_table: &IndexModules,
    module_stack: &mut Vec<ModuleIdx>,
    resolved_exports: &mut FxHashMap<Rstr, SymbolRef>,
  ) {
    if module_stack.contains(&module_idx) {
      return;
    }

    module_stack.push(module_idx);

    let Module::Normal(module) = &module_table[module_idx] else {
      return;
    };

    for module_idx in module.star_export_module_ids() {
      let Module::Normal(dep_module) = &module_table[module_idx] else {
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
          .filter_map(|id| module_table[*id].as_normal())
          .any(|module| module.named_exports.contains_key(exported_name))
        {
          continue;
        }

        if !resolved_exports.contains_key(exported_name) {
          resolved_exports.insert(exported_name.clone(), named_export.referenced);
        }
      }

      Self::add_exports_for_export_star(module_idx, module_table, module_stack, resolved_exports);
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
                let mut canonical_ref =
                  self.symbol_ref_db.canonical_ref_for(member_expr_ref.object_ref);
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
                    resolved_map.insert(
                      member_expr_ref.span,
                      (None, member_expr_ref.props[cursor..].to_vec()),
                    );
                    break;
                  };
                  if !meta.sorted_resolved_exports.contains(&name.to_rstr()) {
                    resolved_map.insert(
                      member_expr_ref.span,
                      (None, member_expr_ref.props[cursor..].to_vec()),
                    );
                    return;
                  };

                  if let Some(chains) = normal_symbol_exports_chain_map.get(export_symbol) {
                    for item in chains {
                      if side_effects_modules.contains(&item.owner) {
                        side_effects_dependency.push(item.owner);
                      }
                    }
                  }
                  ns_symbol_list.push((canonical_ref, name.to_rstr()));
                  canonical_ref = self.symbol_ref_db.canonical_ref_for(*export_symbol);
                  canonical_ref_owner = self.module_table[canonical_ref.owner].as_normal().unwrap();
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

    self.metadata.iter_mut().zip(resolved_meta_data).for_each(
      |(meta, (resolved_map, side_effects_dependency))| {
        meta.resolved_member_expr_refs = resolved_map;
        meta.dependencies.extend(side_effects_dependency);
      },
    );
  }
}
