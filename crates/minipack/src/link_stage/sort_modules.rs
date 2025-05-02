use std::iter;

use minipack_common::{Module, ModuleIdx};
use minipack_utils::rustc_hash::FxHashSetExt;
use rustc_hash::{FxHashMap, FxHashSet};

use super::LinkStage;

#[derive(PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
enum Status {
  ToBeExecuted(ModuleIdx),
  WaitForExit(ModuleIdx),
}

impl LinkStage<'_> {
  pub(crate) fn sort_modules(&mut self) {
    // The runtime module should always be the first module to be executed
    let mut execution_stack = self
      .entry_points
      .iter()
      .rev()
      .map(|entry| Status::ToBeExecuted(entry.id))
      .chain(iter::once(Status::ToBeExecuted(self.runtime_module.id())))
      .collect::<Vec<_>>();

    let mut executed_ids = FxHashSet::with_capacity(self.module_table.len());
    let mut stack_indexes_of_executing_id = FxHashMap::default();

    let mut next_exec_order = 0;
    let mut circular_dependencies = FxHashSet::default();
    let mut sorted_modules = Vec::with_capacity(self.module_table.len());

    while let Some(status) = execution_stack.pop() {
      match status {
        Status::ToBeExecuted(id) => {
          if executed_ids.contains(&id) {
            // Try to check if there is a circular dependency
            if let Some(index) = stack_indexes_of_executing_id.get(&id).copied() {
              // Executing
              let cycles = execution_stack[index..]
                .iter()
                .filter_map(|action| match action {
                  // Only modules with `Status::WaitForExit` are on the execution chain
                  Status::ToBeExecuted(_) => None,
                  Status::WaitForExit(id) => Some(*id),
                })
                .chain(iter::once(id))
                .collect::<Box<[_]>>();
              circular_dependencies.insert(cycles);
            }
          } else {
            executed_ids.insert(id);
            execution_stack.push(Status::WaitForExit(id));
            stack_indexes_of_executing_id.insert(id, execution_stack.len() - 1);

            execution_stack.extend(
              self.module_table[id]
                .import_records()
                .iter()
                .filter(|rec| rec.kind.is_static() && !rec.is_dummy())
                .map(|rec| rec.state)
                .rev()
                .map(Status::ToBeExecuted),
            );
          }
        }
        Status::WaitForExit(id) => {
          match &mut self.module_table[id] {
            Module::Normal(module) => {
              sorted_modules.push(id);
              module.exec_order = next_exec_order;
            }
            Module::External(module) => {
              module.exec_order = next_exec_order;
            }
          }
          next_exec_order += 1;
          stack_indexes_of_executing_id.remove(&id);
        }
      }
    }

    if !circular_dependencies.is_empty() {
      for cycle in circular_dependencies {
        let paths = cycle
          .iter()
          .copied()
          .filter_map(|id| self.module_table[id].as_normal())
          .map(|module| module.id.to_string())
          .collect::<Vec<_>>();

        self.warnings.push(anyhow::anyhow!("Circular dependency: {}.", paths.join(" -> ")));
      }
    }

    self.sorted_modules = sorted_modules;
  }
}
