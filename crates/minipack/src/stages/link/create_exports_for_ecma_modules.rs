use minipack_common::{
  dynamic_import_usage::DynamicImportExportsUsage, EntryPoint, ExportsKind, ModuleIdx,
  NormalModule, NormalizedBundlerOptions, OutputFormat, RuntimeModuleBrief, StmtInfo, StmtInfoMeta,
  SymbolRefDb, WrapKind,
};
use rustc_hash::FxHashMap;

use crate::types::linking_metadata::LinkingMetadata;

use super::LinkStage;

pub fn init_entry_point_stmt_info(
  meta: &mut LinkingMetadata,
  entry: &EntryPoint,
  dynamic_import_exports_usage_map: &FxHashMap<ModuleIdx, DynamicImportExportsUsage>,
) {
  let mut referenced_symbols = vec![];

  // Include the wrapper if present
  if !matches!(meta.wrap_kind, WrapKind::None) {
    // If a commonjs module becomes an entry point while targeting esm, we need to at least add a `export default require_foo();`
    // statement as some kind of syntax sugar. So users won't need to manually create a proxy file with `export default require('./foo.cjs')` in it.
    referenced_symbols.push(meta.wrapper_ref.unwrap());
  }

  referenced_symbols.extend(
    meta
      .referenced_canonical_exports_symbols(entry.id, entry.kind, dynamic_import_exports_usage_map)
      .map(|(_, resolved_export)| resolved_export.symbol_ref),
  );
  // Entry chunk need to generate exports, so we need reference to all exports to make sure they are included in tree-shaking.

  meta.referenced_symbols_by_entry_point_chunk.extend(referenced_symbols);
}

fn create_wrapper(
  module: &mut NormalModule,
  linking_info: &mut LinkingMetadata,
  symbols: &mut SymbolRefDb,
  runtime: &RuntimeModuleBrief,
  options: &NormalizedBundlerOptions,
) {
  match linking_info.wrap_kind {
    // If this is a CommonJS file, we're going to need to generate a wrapper
    // for the CommonJS closure. That will end up looking something like this:
    //
    //   var require_foo = __commonJS((exports, module) => {
    //     ...
    //   });
    //
    WrapKind::Cjs => {
      let wrapper_ref = symbols
        .create_facade_root_symbol_ref(module.idx, &format!("require_{}", &module.repr_name));

      let stmt_info = StmtInfo {
        stmt_idx: None,
        declared_symbols: vec![wrapper_ref],
        referenced_symbols: vec![if !options.minify {
          runtime.resolve_symbol("__commonJS").into()
        } else {
          runtime.resolve_symbol("__commonJSMin").into()
        }],
        side_effect: false,
        is_included: false,
        import_records: Vec::new(),
        debug_label: None,
        meta: StmtInfoMeta::default(),
      };

      linking_info.wrapper_stmt_info = Some(module.stmt_infos.add_stmt_info(stmt_info));
      linking_info.wrapper_ref = Some(wrapper_ref);
    }
    // If this is a lazily-initialized ESM file, we're going to need to
    // generate a wrapper for the ESM closure. That will end up looking
    // something like this:
    //
    //   var init_foo = __esm(() => {
    //     ...
    //   });
    //
    WrapKind::Esm => {
      let wrapper_ref =
        symbols.create_facade_root_symbol_ref(module.idx, &format!("init_{}", &module.repr_name));

      let stmt_info = StmtInfo {
        stmt_idx: None,
        declared_symbols: vec![wrapper_ref],
        referenced_symbols: vec![if !options.minify {
          runtime.resolve_symbol("__esm").into()
        } else {
          runtime.resolve_symbol("__esmMin").into()
        }],
        side_effect: false,
        is_included: false,
        import_records: Vec::new(),
        debug_label: None,
        meta: StmtInfoMeta::default(),
      };

      linking_info.wrapper_stmt_info = Some(module.stmt_infos.add_stmt_info(stmt_info));
      linking_info.wrapper_ref = Some(wrapper_ref);
    }
    WrapKind::None => {}
  }
}

impl LinkStage<'_> {
  pub(crate) fn create_exports_for_ecma_modules(&mut self) {
    self.modules.iter_mut().filter_map(|m| m.as_normal_mut()).for_each(|ecma_module| {
      let linking_info = &mut self.metadata[ecma_module.idx];

      create_wrapper(
        ecma_module,
        linking_info,
        &mut self.symbols,
        &self.runtime_module,
        self.options,
      );
      if let Some(entry) = self.entry_points.iter().find(|entry| entry.id == ecma_module.idx) {
        init_entry_point_stmt_info(linking_info, entry, &self.dyn_import_usage_map);
      }

      // Create facade StmtInfo that declares variables based on the missing exports, so they can participate in the symbol de-conflict and
      // tree-shaking process.
      linking_info.shimmed_missing_exports.iter().for_each(|(_name, symbol_ref)| {
        let stmt_info = StmtInfo {
          stmt_idx: None,
          declared_symbols: vec![*symbol_ref],
          referenced_symbols: vec![],
          side_effect: false,
          is_included: false,
          import_records: Vec::new(),
          debug_label: None,
          meta: StmtInfoMeta::default(),
        };
        ecma_module.stmt_infos.add_stmt_info(stmt_info);
      });

      // Generate export of Module Namespace Object for Namespace Import
      // - Namespace import: https://tc39.es/ecma262/#prod-NameSpaceImport
      // - Module Namespace Object: https://tc39.es/ecma262/#sec-module-namespace-exotic-objects
      // Though Module Namespace Object is created in runtime, as a bundler, we have stimulus the behavior in compile-time and generate a
      // real statement to construct the Module Namespace Object and assign it to a variable.
      // This is only a concept of esm, so no need to care about this in commonjs.
      if matches!(ecma_module.exports_kind, ExportsKind::Esm) {
        let meta = &self.metadata[ecma_module.idx];
        let mut referenced_symbols = vec![];
        let mut declared_symbols = vec![];
        if !meta.is_canonical_exports_empty() {
          referenced_symbols.push(self.runtime_module.resolve_symbol("__export").into());
          referenced_symbols
            .extend(meta.canonical_exports().map(|(_, export)| export.symbol_ref.into()));
        }
        if !meta.star_exports_from_external_modules.is_empty() {
          referenced_symbols.push(self.runtime_module.resolve_symbol("__reExport").into());
          match self.options.format {
            OutputFormat::Esm => {
              meta.star_exports_from_external_modules.iter().copied().for_each(|rec_idx| {
                referenced_symbols.push(ecma_module.import_records[rec_idx].namespace_ref.into());
                declared_symbols.push(ecma_module.import_records[rec_idx].namespace_ref);
              });
            }
            OutputFormat::Cjs => {}
          }
        };
        // Create a StmtInfo to represent the statement that declares and constructs the Module Namespace Object.
        // Corresponding AST for this statement will be created by the finalizer.
        declared_symbols.push(ecma_module.namespace_object_ref);
        let namespace_stmt_info = StmtInfo {
          stmt_idx: None,
          declared_symbols,
          referenced_symbols,
          side_effect: false,
          is_included: false,
          import_records: Vec::new(),
          debug_label: None,
          meta: StmtInfoMeta::default(),
        };
        ecma_module.stmt_infos.replace_namespace_stmt_info(namespace_stmt_info);
      }
    });
  }
}
