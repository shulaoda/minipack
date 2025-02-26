use minipack_common::{ExportsKind, Module, ModuleIdx, WrapKind};
use oxc_index::IndexVec;

use crate::{
  link_stage::create_exports_for_ecma_modules::create_wrapper,
  types::{IndexModules, linking_metadata::LinkingMetadataVec},
};

use super::LinkStage;

struct Context<'a> {
  pub visited_modules: &'a mut IndexVec<ModuleIdx, bool>,
  pub linking_infos: &'a mut LinkingMetadataVec,
  pub modules: &'a IndexModules,
}

fn wrap_module_recursively(ctx: &mut Context, target: ModuleIdx) {
  let Module::Normal(module) = &ctx.modules[target] else {
    return;
  };

  if ctx.visited_modules[target] {
    return;
  }
  ctx.visited_modules[target] = true;

  if matches!(ctx.linking_infos[target].wrap_kind, WrapKind::None) {
    ctx.linking_infos[target].wrap_kind = match module.exports_kind {
      ExportsKind::Esm | ExportsKind::None => WrapKind::Esm,
      ExportsKind::CommonJs => WrapKind::Cjs,
    }
  }

  module.import_records.iter().filter(|item| !item.is_dummy()).for_each(|importee| {
    wrap_module_recursively(ctx, importee.resolved_module);
  });
}

fn has_dynamic_exports_due_to_export_star(
  target: ModuleIdx,
  modules: &IndexModules,
  linking_infos: &mut LinkingMetadataVec,
  visited_modules: &mut IndexVec<ModuleIdx, bool>,
) -> bool {
  if visited_modules[target] {
    return linking_infos[target].has_dynamic_exports;
  }
  visited_modules[target] = true;

  let has_dynamic_exports = if let Some(module) = modules[target].as_normal() {
    matches!(module.exports_kind, ExportsKind::CommonJs)
      || module.star_export_module_ids().any(|importee_id| {
        target != importee_id
          && has_dynamic_exports_due_to_export_star(
            importee_id,
            modules,
            linking_infos,
            visited_modules,
          )
      })
  } else {
    true
  };

  if has_dynamic_exports {
    linking_infos[target].has_dynamic_exports = true;
  }

  linking_infos[target].has_dynamic_exports
}

impl LinkStage<'_> {
  pub(crate) fn wrap_modules(&mut self) {
    let mut visited_modules_for_wrapping = oxc_index::index_vec![false; self.modules.len()];

    let mut visited_modules_for_dynamic_exports = oxc_index::index_vec![false; self.modules.len()];

    debug_assert!(!self.sorted_modules.is_empty());

    let sorted_module_iter =
      self.sorted_modules.iter().filter_map(|idx| self.modules[*idx].as_normal());

    for module in sorted_module_iter {
      let need_to_wrap =
        matches!(self.metadata[module.idx].wrap_kind, WrapKind::Cjs | WrapKind::Esm);

      visited_modules_for_wrapping[module.idx] = true;
      module.import_records.iter().filter(|rec| !rec.is_dummy()).for_each(|rec| {
        let Module::Normal(importee) = &self.modules[rec.resolved_module] else {
          return;
        };
        if matches!(importee.exports_kind, ExportsKind::CommonJs) || need_to_wrap {
          wrap_module_recursively(
            &mut Context {
              visited_modules: &mut visited_modules_for_wrapping,
              linking_infos: &mut self.metadata,
              modules: &self.modules,
            },
            importee.idx,
          );
        }
      });

      if module.has_star_export() {
        has_dynamic_exports_due_to_export_star(
          module.idx,
          &self.modules,
          &mut self.metadata,
          &mut visited_modules_for_dynamic_exports,
        );
      }
    }
    self.modules.iter_mut().filter_map(|m| m.as_normal_mut()).for_each(|ecma_module| {
      let linking_info = &mut self.metadata[ecma_module.idx];
      create_wrapper(
        ecma_module,
        linking_info,
        &mut self.symbols,
        &self.runtime_module,
        self.options,
      );
    });
  }
}
