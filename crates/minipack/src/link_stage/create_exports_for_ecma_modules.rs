use minipack_common::{OutputFormat, StmtInfo};

use super::LinkStage;

impl LinkStage<'_> {
  pub(crate) fn create_exports_for_ecma_modules(&mut self) {
    self.module_table.iter_mut().filter_map(|m| m.as_normal_mut()).for_each(|normal_module| {
      let linking_info = &mut self.metadata[normal_module.idx];

      if let Some(entry) = self.entry_points.iter().find(|entry| entry.id == normal_module.idx) {
        let referenced_symbols = linking_info
          .referenced_canonical_exports_symbols(entry.id, entry.kind, &self.dyn_import_usage_map)
          .map(|(_, resolved_export)| resolved_export.symbol_ref)
          .collect::<Vec<_>>();

        linking_info.referenced_symbols_by_entry_point_chunk.extend(referenced_symbols);
      }

      // Generate export of Module Namespace Object for Namespace Import
      // - Namespace import: https://tc39.es/ecma262/#prod-NameSpaceImport
      // - Module Namespace Object: https://tc39.es/ecma262/#sec-module-namespace-exotic-objects
      // Though Module Namespace Object is created in runtime, as a bundler, we have stimulus the behavior in compile-time and generate a
      // real statement to construct the Module Namespace Object and assign it to a variable.
      // This is only a concept of esm, so no need to care about this in commonjs.
      let mut declared_symbols = vec![];
      let mut referenced_symbols = vec![];

      let meta = &self.metadata[normal_module.idx];

      if !meta.is_canonical_exports_empty() {
        referenced_symbols.push(self.runtime_module.resolve_symbol("__export").into());
        referenced_symbols
          .extend(meta.canonical_exports().map(|(_, export)| export.symbol_ref.into()));
      }

      if !meta.star_exports_from_external_modules.is_empty() {
        referenced_symbols.push(self.runtime_module.resolve_symbol("__reExport").into());

        if let OutputFormat::Esm = self.options.format {
          meta.star_exports_from_external_modules.iter().copied().for_each(|idx| {
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
