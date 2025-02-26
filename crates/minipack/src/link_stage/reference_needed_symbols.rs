use std::{ptr::addr_of, sync::Mutex};

use minipack_common::{
  ImportKind, ImportRecordIdx, ImportRecordMeta, Module, OutputFormat,
  side_effects::DeterminedSideEffects,
};
use minipack_utils::{
  concat_string,
  ecmascript::legitimize_identifier_name,
  rayon::{IntoParallelRefIterator, ParallelIterator},
};

use super::LinkStage;

impl LinkStage<'_> {
  pub(crate) fn reference_needed_symbols(&mut self) {
    let symbols = Mutex::new(&mut self.symbols);
    let record_meta_update_pending_pairs_list = self
      .modules
      .par_iter()
      .filter_map(Module::as_normal)
      .map(|importer| {
        let record_meta_pairs: Vec<(ImportRecordIdx, ImportRecordMeta)> = vec![];
        let importer_idx = importer.idx;
        // safety: No race conditions here:
        // - Mutating on `stmt_infos` is isolated in threads for each module
        // - Mutating on `stmt_infos` doesn't rely on other mutating operations of other modules
        // - Mutating and parallel reading is in different memory locations
        let stmt_infos = unsafe { &mut *(addr_of!(importer.stmt_infos).cast_mut()) };
        let importer_side_effect = unsafe { &mut *(addr_of!(importer.side_effects).cast_mut()) };

        // store the symbol reference to the declared statement index
        stmt_infos.infos.iter_mut_enumerated().for_each(|(_stmt_idx, stmt_info)| {
          stmt_info.import_records.iter().for_each(|rec_id| {
            let rec = &importer.import_records[*rec_id];
            if rec.is_dummy() {
              return;
            }

            match &self.modules[rec.resolved_module] {
              Module::External(importee) => {
                // Make sure symbols from external modules are included and de_conflicted
                if rec.kind == ImportKind::Import {
                  let is_reexport_all = rec.meta.contains(ImportRecordMeta::IS_EXPORT_STAR);
                  if is_reexport_all {
                    // export * from 'external' would be just removed. So it references nothing.
                    rec.namespace_ref.set_name(
                      &mut symbols.lock().unwrap(),
                      &concat_string!("import_", legitimize_identifier_name(&importee.name)),
                    );
                  } else {
                    // import ... from 'external' or export ... from 'external'
                    if matches!(self.options.format, OutputFormat::Cjs)
                      && !rec.meta.contains(ImportRecordMeta::IS_PLAIN_IMPORT)
                    {
                      stmt_info.side_effect = true;
                      stmt_info
                        .referenced_symbols
                        .push(self.runtime_module.resolve_symbol("__toESM").into());
                    }
                  }
                }
              }
              Module::Normal(importee) => {
                match rec.kind {
                  ImportKind::Import => {
                    let is_reexport_all = rec.meta.contains(ImportRecordMeta::IS_EXPORT_STAR);
                    // for case:
                    // ```js
                    // // index.js
                    // export * from './foo'; /* importee wrap kind is `none`, but since `foo` has dynamic_export, we need to preserve the `__reExport(index_exports, foo_ns)` */
                    //
                    // // foo.js
                    // export * from './bar' /* importee wrap kind is `cjs`, preserve by
                    // default*/
                    //
                    // // bar.js
                    // module.exports = 1000
                    // ```
                    if is_reexport_all {
                      let meta = &self.metadata[importee.idx];
                      if meta.has_dynamic_exports {
                        *importer_side_effect = DeterminedSideEffects::Analyzed(true);
                        stmt_info.side_effect = true;
                        stmt_info
                          .referenced_symbols
                          .push(self.runtime_module.resolve_symbol("__reExport").into());
                        stmt_info.referenced_symbols.push(importer.namespace_object_ref.into());
                        stmt_info.referenced_symbols.push(importee.namespace_object_ref.into());
                      }
                    }
                  }
                  ImportKind::AtImport => {
                    unreachable!("A Js module would never import a CSS module via `@import`");
                  }
                  ImportKind::UrlImport => {
                    unreachable!("A Js module would never import a CSS module via `url()`");
                  }
                  _ => {}
                }
              }
            }
          });
        });
        (importer_idx, record_meta_pairs)
      })
      .collect::<Vec<_>>();

    // merge import_record.meta
    for (module_idx, record_meta_pairs) in record_meta_update_pending_pairs_list {
      let Some(module) = self.modules[module_idx].as_normal_mut() else {
        continue;
      };
      for (rec_id, meta) in record_meta_pairs {
        module.import_records[rec_id].meta |= meta;
      }
    }
  }
}
