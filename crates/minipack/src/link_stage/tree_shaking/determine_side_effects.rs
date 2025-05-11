use minipack_common::{Module, ModuleIdx, side_effects::DeterminedSideEffects};
use oxc_index::IndexVec;

#[derive(Debug, Clone, Copy)]
enum SideEffectCache {
  None,
  Visited,
  Cache(DeterminedSideEffects),
}

impl crate::link_stage::LinkStage {
  pub fn determine_side_effects(&mut self) {
    let mut side_effects_cache =
      oxc_index::index_vec![SideEffectCache::None; self.module_table.len()];
    for idx in 0..self.module_table.len() {
      let idx = ModuleIdx::new(idx);
      let side_effects = self.determine_side_effects_for_module(idx, &mut side_effects_cache);
      if let Module::Normal(module) = &mut self.module_table[idx] {
        module.side_effects = side_effects;
      }
    }
  }

  fn determine_side_effects_for_module(
    &self,
    module_idx: ModuleIdx,
    cache: &mut IndexVec<ModuleIdx, SideEffectCache>,
  ) -> DeterminedSideEffects {
    let module = &self.module_table[module_idx];

    match cache[module_idx] {
      SideEffectCache::None => {
        cache[module_idx] = SideEffectCache::Visited;
      }
      SideEffectCache::Visited => {
        return *module.side_effects();
      }
      SideEffectCache::Cache(v) => {
        return v;
      }
    }

    let module_side_effects = *module.side_effects();
    if let DeterminedSideEffects::Analyzed(false) = module_side_effects {
      if let Module::Normal(module) = module {
        let side_effects =
          DeterminedSideEffects::Analyzed(module.import_records.iter().any(|rec| {
            self.determine_side_effects_for_module(rec.state, cache).has_side_effects()
          }));

        cache[module_idx] = SideEffectCache::Cache(side_effects);

        return side_effects;
      }
    }
    module_side_effects
  }
}
