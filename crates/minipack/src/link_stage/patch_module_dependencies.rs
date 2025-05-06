use minipack_common::{Module, SymbolOrMemberExprRef};

use super::LinkStage;

impl LinkStage<'_> {
  pub(crate) fn patch_module_dependencies(&mut self) {
    self.metadata.iter_mut_enumerated().for_each(|(idx, meta)| {
      // Symbols from runtime are referenced by bundler not import statements.
      meta.referenced_symbols_by_entry_point_chunk.iter().for_each(|symbol_ref| {
        meta.dependencies.insert(self.symbols.canonical_ref_for(*symbol_ref).owner);
      });

      // We need this step to include the runtime module, if there are symbols of it.
      if let Module::Normal(module) = &self.module_table[idx] {
        module.stmt_infos.iter().for_each(|stmt_info| {
          if stmt_info.is_included {
            for reference_ref in &stmt_info.referenced_symbols {
              let sym_ref = match reference_ref {
                SymbolOrMemberExprRef::Symbol(sym_ref) => *sym_ref,
                SymbolOrMemberExprRef::MemberExpr(member_expr) => {
                  let resolved_map = &meta.resolved_member_expr_refs;
                  if let Some(sym_ref) = member_expr.resolved_symbol_ref(resolved_map) {
                    sym_ref
                  } else {
                    continue;
                  }
                }
              };

              let canonical_ref = self.symbols.canonical_ref_for(sym_ref);
              let symbol = self.symbols.get(canonical_ref);
              if let Some(ns) = &symbol.namespace_alias {
                meta.dependencies.insert(ns.namespace_ref.owner);
              }
              meta.dependencies.insert(canonical_ref.owner);
            }
          }
        });
      }
    });
  }
}
