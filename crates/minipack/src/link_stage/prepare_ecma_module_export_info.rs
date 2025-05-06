use minipack_common::{OutputFormat, StmtInfo};

use super::LinkStage;

impl LinkStage<'_> {
  pub(crate) fn prepare_ecma_module_export_info(&mut self) {
    self.process_entry_point_module_exports();
    self.create_module_namespace_object_stmt_info();
  }

  fn process_entry_point_module_exports(&mut self) {
    for entry in &self.entry_points {
      if self.module_table[entry.idx].is_normal() {
        let linking_info = &mut self.metadata[entry.idx];
        let iter = linking_info
          .sorted_and_non_ambiguous_resolved_exports
          .iter()
          .map(|name| linking_info.resolved_exports[name].symbol_ref);

        linking_info.referenced_symbols_by_entry_point_chunk.extend(iter);
      }
    }
  }

  fn create_module_namespace_object_stmt_info(&mut self) {
    self.module_table.iter_mut().filter_map(|m| m.as_normal_mut()).for_each(|normal_module| {
      let mut declared_symbols = vec![];
      let mut referenced_symbols = vec![];

      let linking_info = &self.metadata[normal_module.idx];

      if !linking_info.sorted_and_non_ambiguous_resolved_exports.is_empty() {
        referenced_symbols.push(self.runtime_module.resolve_symbol("__export").into());
        referenced_symbols.extend(
          linking_info
            .sorted_and_non_ambiguous_resolved_exports
            .iter()
            .map(|name| linking_info.resolved_exports[name].symbol_ref.into()),
        );
      }

      if !linking_info.star_exports_from_external_modules.is_empty() {
        referenced_symbols.push(self.runtime_module.resolve_symbol("__reExport").into());
        if let OutputFormat::Esm = self.options.format {
          linking_info.star_exports_from_external_modules.iter().copied().for_each(|idx| {
            declared_symbols.push(normal_module.import_records[idx].namespace_ref);
            referenced_symbols.push(normal_module.import_records[idx].namespace_ref.into());
          });
        }
      };

      // Create a StmtInfo to represent the statement that declares and constructs the Module Namespace Object.
      // Corresponding AST for this statement will be created by the finalizer.
      declared_symbols.push(normal_module.namespace_object_ref);
      normal_module.stmt_infos.replace_namespace_stmt_info(StmtInfo {
        declared_symbols,
        referenced_symbols,
        ..Default::default()
      });
    });
  }
}
