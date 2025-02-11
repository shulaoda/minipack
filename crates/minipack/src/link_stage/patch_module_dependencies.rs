use minipack_common::{Module, SymbolOrMemberExprRef};

use super::LinkStage;

impl LinkStage<'_> {
  pub(crate) fn patch_module_dependencies(&mut self) {
    self.metadata.iter_mut_enumerated().for_each(|(module_idx, meta)| {
      // Symbols from runtime are referenced by bundler not import statements.
      meta.referenced_symbols_by_entry_point_chunk.iter().for_each(|symbol_ref| {
        let canonical_ref = self.symbols.canonical_ref_for(*symbol_ref);
        meta.dependencies.insert(canonical_ref.owner);
      });

      let Module::Normal(module) = &self.modules[module_idx] else {
        return;
      };

      module.stmt_infos.iter().filter(|stmt_info| stmt_info.is_included).for_each(|stmt_info| {
        // We need this step to include the runtime module, if there are symbols of it.
        stmt_info.referenced_symbols.iter().for_each(|reference_ref| {
          match reference_ref {
            SymbolOrMemberExprRef::Symbol(sym_ref) => {
              let canonical_ref = self.symbols.canonical_ref_for(*sym_ref);
              meta.dependencies.insert(canonical_ref.owner);
              let symbol = self.symbols.get(canonical_ref);
              if let Some(ns) = &symbol.namespace_alias {
                meta.dependencies.insert(ns.namespace_ref.owner);
              }
            }
            SymbolOrMemberExprRef::MemberExpr(member_expr) => {
              if let Some(sym_ref) =
                member_expr.resolved_symbol_ref(&meta.resolved_member_expr_refs)
              {
                let canonical_ref = self.symbols.canonical_ref_for(sym_ref);
                meta.dependencies.insert(canonical_ref.owner);
                let symbol = self.symbols.get(canonical_ref);
                if let Some(ns) = &symbol.namespace_alias {
                  meta.dependencies.insert(ns.namespace_ref.owner);
                }
              } else {
                // `None` means the member expression resolve to a ambiguous export, which means it actually resolve to nothing.
                // It would be rewrite to `undefined` in the final code, so we don't need to include anything to make `undefined` work.
              }
            }
          };
        });
      });
    });
  }
}
