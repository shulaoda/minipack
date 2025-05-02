use minipack_common::ModuleIdx;
use oxc_index::IndexVec;

use crate::types::{IndexModules, linking_metadata::LinkingMetadataVec};

use super::LinkStage;

fn has_dynamic_exports_due_to_export_star(
  target: ModuleIdx,
  module_table: &IndexModules,
  linking_infos: &mut LinkingMetadataVec,
  visited_modules: &mut IndexVec<ModuleIdx, bool>,
) -> bool {
  if visited_modules[target] {
    return linking_infos[target].has_dynamic_exports;
  }
  visited_modules[target] = true;

  let has_dynamic_exports = module_table[target].as_normal().is_none_or(|module| {
    module.star_export_module_ids().any(|importee_id| {
      target != importee_id
        && has_dynamic_exports_due_to_export_star(
          importee_id,
          module_table,
          linking_infos,
          visited_modules,
        )
    })
  });

  if has_dynamic_exports {
    linking_infos[target].has_dynamic_exports = true;
  }

  linking_infos[target].has_dynamic_exports
}

impl LinkStage<'_> {
  pub(crate) fn wrap_modules(&mut self) {
    let mut visited_modules_for_wrapping = oxc_index::index_vec![false; self.module_table.len()];
    let mut visited_modules_for_dynamic_exports =
      oxc_index::index_vec![false; self.module_table.len()];

    debug_assert!(!self.sorted_modules.is_empty());

    let sorted_module_iter =
      self.sorted_modules.iter().filter_map(|idx| self.module_table[*idx].as_normal());

    for module in sorted_module_iter {
      visited_modules_for_wrapping[module.idx] = true;

      if module.has_star_export() {
        has_dynamic_exports_due_to_export_star(
          module.idx,
          &self.module_table,
          &mut self.metadata,
          &mut visited_modules_for_dynamic_exports,
        );
      }
    }
  }
}
