use minipack_common::{
  side_effects::DeterminedSideEffects, Module, ModuleIdx, ModuleType, NormalModule, StmtInfoIdx,
  SymbolOrMemberExprRef, SymbolRef, SymbolRefDb,
};
use minipack_utils::rayon::{IntoParallelRefMutIterator, ParallelIterator};
use oxc_index::IndexVec;
use rustc_hash::FxHashSet;

use crate::types::{linking_metadata::LinkingMetadataVec, IndexModules};

use super::LinkStage;

struct Context<'a> {
  modules: &'a IndexModules,
  symbols: &'a SymbolRefDb,
  is_included_vec: &'a mut IndexVec<ModuleIdx, IndexVec<StmtInfoIdx, bool>>,
  is_module_included_vec: &'a mut IndexVec<ModuleIdx, bool>,
  tree_shaking: bool,
  runtime_id: ModuleIdx,
  metas: &'a LinkingMetadataVec,
  used_symbol_refs: &'a mut FxHashSet<SymbolRef>,
}

/// if no export is used, and the module has no side effects, the module should not be included
fn include_module(ctx: &mut Context, module: &NormalModule) {
  fn forcefully_include_all_statements(ctx: &mut Context, module: &NormalModule) {
    // Skip the first statement, which is the namespace object. It should be included only if it is used no matter
    // tree shaking is enabled or not.
    module.stmt_infos.iter_enumerated().skip(1).for_each(|(stmt_info_id, _stmt_info)| {
      include_statement(ctx, module, stmt_info_id);
    });
  }

  let is_included = ctx.is_module_included_vec[module.idx];
  if is_included {
    return;
  }
  ctx.is_module_included_vec[module.idx] = true;

  if module.idx == ctx.runtime_id {
    // runtime module has no side effects and it's statements should be included
    // by other modules's references.
    return;
  }

  let forced_no_treeshake = matches!(module.side_effects, DeterminedSideEffects::NoTreeshake);
  if ctx.tree_shaking && !forced_no_treeshake {
    module.stmt_infos.iter_enumerated().for_each(|(stmt_info_id, stmt_info)| {
      // No need to handle the first statement specially, which is the namespace object, because it doesn't have side effects and will only be included if it is used.
      let bail_eval = module.meta.has_eval()
        && !stmt_info.declared_symbols.is_empty()
        && stmt_info_id.index() != 0;
      if stmt_info.side_effect || bail_eval {
        include_statement(ctx, module, stmt_info_id);
      }
    });
  } else {
    forcefully_include_all_statements(ctx, module);
  }

  let module_meta = &ctx.metas[module.idx];

  // Include imported modules for its side effects
  module_meta.dependencies.iter().copied().for_each(|dependency_idx| {
    match &ctx.modules[dependency_idx] {
      Module::Normal(importee) => {
        if !ctx.tree_shaking || importee.side_effects.has_side_effects() {
          include_module(ctx, importee);
        }
      }
      Module::External(_) => {}
    }
  });
  if module.meta.has_eval() && matches!(module.module_type, ModuleType::Js | ModuleType::Jsx) {
    module.named_imports.keys().for_each(|symbol| {
      include_symbol(ctx, *symbol);
    });
  }
}

fn include_symbol(ctx: &mut Context, symbol_ref: SymbolRef) {
  let mut canonical_ref = ctx.symbols.canonical_ref_for(symbol_ref);
  let canonical_ref_symbol = ctx.symbols.get(canonical_ref);
  if let Some(namespace_alias) = &canonical_ref_symbol.namespace_alias {
    canonical_ref = namespace_alias.namespace_ref;
  }

  ctx.used_symbol_refs.insert(canonical_ref);

  if let Module::Normal(module) = &ctx.modules[canonical_ref.owner] {
    include_module(ctx, module);
    module.stmt_infos.declared_stmts_by_symbol(&canonical_ref).iter().copied().for_each(
      |stmt_info_id| {
        include_statement(ctx, module, stmt_info_id);
      },
    );
  }
}

fn include_statement(ctx: &mut Context, module: &NormalModule, stmt_info_id: StmtInfoIdx) {
  let is_included = &mut ctx.is_included_vec[module.idx][stmt_info_id];

  if *is_included {
    return;
  }

  let stmt_info = module.stmt_infos.get(stmt_info_id);

  // include the statement itself
  *is_included = true;

  stmt_info.referenced_symbols.iter().for_each(|reference_ref| match reference_ref {
    SymbolOrMemberExprRef::Symbol(symbol_ref) => {
      include_symbol(ctx, *symbol_ref);
    }
    SymbolOrMemberExprRef::MemberExpr(member_expr) => {
      if let Some(symbol) =
        member_expr.resolved_symbol_ref(&ctx.metas[module.idx].resolved_member_expr_refs)
      {
        include_symbol(ctx, symbol);
      }
    }
  });
}

impl LinkStage<'_> {
  pub fn include_statements(&mut self) {
    let mut is_included_vec: IndexVec<ModuleIdx, IndexVec<StmtInfoIdx, bool>> = self
      .modules
      .iter()
      .map(|m| {
        m.as_normal().map_or(IndexVec::default(), |m| {
          m.stmt_infos.iter().map(|_| false).collect::<IndexVec<StmtInfoIdx, _>>()
        })
      })
      .collect::<IndexVec<ModuleIdx, _>>();

    let mut is_module_included_vec: IndexVec<ModuleIdx, bool> =
      oxc_index::index_vec![false; self.modules.len()];

    let context = &mut Context {
      modules: &self.modules,
      symbols: &self.symbols,
      is_included_vec: &mut is_included_vec,
      is_module_included_vec: &mut is_module_included_vec,
      tree_shaking: true,
      runtime_id: self.runtime_module.id(),
      // used_exports_info_vec: &mut used_exports_info_vec,
      metas: &self.metadata,
      used_symbol_refs: &mut self.used_symbol_refs,
    };

    self.entry_points.iter().for_each(|entry| {
      let module = match &self.modules[entry.id] {
        Module::Normal(module) => module,
        Module::External(_module) => {
          // Case: import('external').
          return;
        }
      };
      let meta = &self.metadata[entry.id];
      meta.referenced_symbols_by_entry_point_chunk.iter().for_each(|symbol_ref| {
        include_symbol(context, *symbol_ref);
      });
      include_module(context, module);
    });

    self.modules.par_iter_mut().filter_map(Module::as_normal_mut).for_each(|module| {
      let idx = module.idx;
      module.meta.set_included(is_module_included_vec[idx]);
      is_included_vec[module.idx].iter_enumerated().for_each(|(stmt_info_id, is_included)| {
        module.stmt_infos.get_mut(stmt_info_id).is_included = *is_included;
      });
    });
  }
}
