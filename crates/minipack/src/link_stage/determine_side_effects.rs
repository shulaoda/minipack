use minipack_common::{Module, ModuleIdx, side_effects::DeterminedSideEffects};
use oxc_index::IndexVec;

use super::LinkStage;

#[derive(Debug, Clone, Copy)]
enum SideEffectCache {
  None,
  Visited,
  Cache(DeterminedSideEffects),
}

impl LinkStage<'_> {
  pub(crate) fn determine_side_effects(&mut self) {
    let mut index_side_effects_cache =
      oxc_index::index_vec![SideEffectCache::None; self.module_table.len()];
    let index_module_side_effects = self
      .module_table
      .iter()
      .map(|module| {
        self.determine_side_effects_for_module(module.idx(), &mut index_side_effects_cache)
      })
      .collect::<Vec<_>>();

    self.module_table.iter_mut().zip(index_module_side_effects).for_each(
      |(module, side_effects)| {
        if let Module::Normal(module) = module {
          module.side_effects = side_effects;
        }
      },
    );
  }

  fn determine_side_effects_for_module(
    &self,
    idx: ModuleIdx,
    cache: &mut IndexVec<ModuleIdx, SideEffectCache>,
  ) -> DeterminedSideEffects {
    let module = &self.module_table[idx];

    match &mut cache[idx] {
      SideEffectCache::None => {
        cache[idx] = SideEffectCache::Visited;
      }
      SideEffectCache::Visited => {
        return *module.side_effects();
      }
      SideEffectCache::Cache(v) => {
        return *v;
      }
    }

    let ret = match *module.side_effects() {
      // should keep as is if the side effects is derived from package.json, it is already
      // true or `no-treeshake`
      DeterminedSideEffects::UserDefined(_) | DeterminedSideEffects::NoTreeshake => {
        *module.side_effects()
      }
      DeterminedSideEffects::Analyzed(v) if v => *module.side_effects(),
      // this branch means the side effects of the module is analyzed `false`
      DeterminedSideEffects::Analyzed(_) => match module {
        Module::Normal(module) => DeterminedSideEffects::Analyzed(
          module.import_records.iter().filter(|rec| !rec.is_dummy()).any(|import_record| {
            self
              .determine_side_effects_for_module(import_record.resolved_module, cache)
              .has_side_effects()
          }),
        ),
        Module::External(module) => module.side_effects,
      },
    };

    cache[idx] = SideEffectCache::Cache(ret);

    ret
  }
}
