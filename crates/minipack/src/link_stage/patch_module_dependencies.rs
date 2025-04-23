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

      let Module::Normal(module) = &self.module_table[module_idx] else {
        return;
      };

      module.stmt_infos.iter().for_each(|stmt_info| {
        if stmt_info.is_included {
          // We need this step to include the runtime module, if there are symbols of it.
          for reference_ref in &stmt_info.referenced_symbols {
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
                }
              }
            };
          }
        }
      });
    });
  }
}
