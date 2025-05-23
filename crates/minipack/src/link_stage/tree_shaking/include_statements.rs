use minipack_common::{
  EcmaViewMeta, Module, ModuleIdx, NormalModule, StmtInfoIdx, SymbolOrMemberExprRef, SymbolRef,
  SymbolRefDb,
};
use minipack_utils::rayon::{IntoParallelRefMutIterator, ParallelIterator};
use oxc_index::IndexVec;
use rustc_hash::FxHashSet;

use crate::types::{IndexModules, LinkingMetadataVec};

struct Context<'a> {
  module_table: &'a IndexModules,
  symbol_ref_db: &'a SymbolRefDb,
  is_included_vec: &'a mut IndexVec<ModuleIdx, IndexVec<StmtInfoIdx, bool>>,
  is_module_included_vec: &'a mut IndexVec<ModuleIdx, bool>,
  runtime_id: ModuleIdx,
  metadata: &'a LinkingMetadataVec,
  used_symbol_refs: &'a mut FxHashSet<SymbolRef>,
}

impl crate::link_stage::LinkStage {
  pub fn include_statements(&mut self) {
    let mut is_included_vec = self
      .module_table
      .iter()
      .map(|m| {
        m.as_normal().map_or(IndexVec::default(), |m| {
          m.stmt_infos.iter().map(|_| false).collect::<IndexVec<StmtInfoIdx, _>>()
        })
      })
      .collect::<IndexVec<ModuleIdx, _>>();

    let mut is_module_included_vec = oxc_index::index_vec![false; self.module_table.len()];

    let context = &mut Context {
      module_table: &self.module_table,
      symbol_ref_db: &self.symbol_ref_db,
      is_included_vec: &mut is_included_vec,
      is_module_included_vec: &mut is_module_included_vec,
      runtime_id: self.runtime_module.idx,
      metadata: &self.metadata,
      used_symbol_refs: &mut self.used_symbol_refs,
    };

    self.entry_points.iter().for_each(|entry| {
      if let Module::Normal(module) = &self.module_table[entry.idx] {
        let meta = &self.metadata[entry.idx];
        meta.referenced_symbols_by_entry_point_chunk.iter().for_each(|symbol_ref| {
          include_symbol(context, *symbol_ref);
        });
        include_module(context, module);
      }
    });

    self.module_table.par_iter_mut().for_each(|module| {
      if let Module::Normal(module) = module {
        let value = is_module_included_vec[module.idx];
        module.meta.set(EcmaViewMeta::INCLUDED, value);
        is_included_vec[module.idx].iter_enumerated().for_each(|(stmt_info_id, is_included)| {
          module.stmt_infos.get_mut(stmt_info_id).is_included = *is_included;
        });
      }
    });
  }
}

/// if no export is used, and the module has no side effects, the module should not be included
fn include_module(ctx: &mut Context, module: &NormalModule) {
  if ctx.is_module_included_vec[module.idx] {
    return;
  }

  ctx.is_module_included_vec[module.idx] = true;

  if module.idx == ctx.runtime_id {
    return;
  }

  module.stmt_infos.iter_enumerated().for_each(|(stmt_info_id, stmt_info)| {
    if stmt_info.side_effect {
      include_statement(ctx, module, stmt_info_id);
    }
  });

  // Include imported modules for its side effects
  ctx.metadata[module.idx].dependencies.iter().copied().for_each(|dependency_idx| {
    match &ctx.module_table[dependency_idx] {
      Module::Normal(importee) => {
        if importee.side_effects.has_side_effects() {
          include_module(ctx, importee);
        }
      }
      Module::External(_) => {}
    }
  });
}

fn include_symbol(ctx: &mut Context, symbol_ref: SymbolRef) {
  let mut canonical_ref = ctx.symbol_ref_db.canonical_ref_for(symbol_ref);
  let canonical_ref_symbol = ctx.symbol_ref_db.get(canonical_ref);
  if let Some(namespace_alias) = &canonical_ref_symbol.namespace_alias {
    canonical_ref = namespace_alias.namespace_ref;
  }

  ctx.used_symbol_refs.insert(canonical_ref);

  let mut include_symbol_impl = |symbol_ref: SymbolRef| {
    if let Module::Normal(module) = &ctx.module_table[symbol_ref.owner] {
      include_module(ctx, module);
      module.stmt_infos.declared_stmts_by_symbol(&symbol_ref).iter().copied().for_each(
        |stmt_info_id| {
          include_statement(ctx, module, stmt_info_id);
        },
      );
    }
  };

  // `symbol_ref` is symbol itself in current module
  include_symbol_impl(symbol_ref);
  // Skip if symbol_ref is the same as canonical_ref.
  if symbol_ref != canonical_ref {
    // `canonical_ref` is the symbol that imports from the other module.
    include_symbol_impl(canonical_ref);
  }
}

fn include_statement(ctx: &mut Context, module: &NormalModule, stmt_info_id: StmtInfoIdx) {
  let is_included = &mut ctx.is_included_vec[module.idx][stmt_info_id];

  if *is_included {
    return;
  }

  // include the statement itself
  *is_included = true;

  let resolved_map = &ctx.metadata[module.idx].resolved_member_expr_refs;
  module.stmt_infos.get(stmt_info_id).referenced_symbols.iter().for_each(|reference_ref| {
    match reference_ref {
      SymbolOrMemberExprRef::Symbol(symbol_ref) => {
        include_symbol(ctx, *symbol_ref);
      }
      SymbolOrMemberExprRef::MemberExpr(member_expr) => {
        if let Some(symbol) = member_expr.resolved_symbol_ref(resolved_map) {
          include_symbol(ctx, symbol);
        }
      }
    }
  });
}
