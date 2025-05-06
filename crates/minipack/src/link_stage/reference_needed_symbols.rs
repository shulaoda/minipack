use std::{ptr::addr_of, sync::Mutex};

use minipack_common::{ImportKind, ImportRecordMeta, Module, OutputFormat};
use minipack_utils::{
  concat_string,
  rayon::{IntoParallelRefIterator, ParallelIterator},
};

use crate::utils::ecmascript::legitimize_identifier_name;

use super::LinkStage;

impl LinkStage<'_> {
  pub(crate) fn reference_needed_symbols(&mut self) {
    let symbols = Mutex::new(&mut self.symbols);
    self.module_table.par_iter().for_each(|module| {
      let Module::Normal(importer) = module else { return };

      // safety: No race conditions here:
      // - Mutating on `stmt_infos` is isolated in threads for each module
      // - Mutating on `stmt_infos` doesn't rely on other mutating operations of other modules
      // - Mutating and parallel reading is in different memory locations
      let stmt_infos = unsafe { &mut *(addr_of!(importer.stmt_infos).cast_mut()) };

      // store the symbol reference to the declared statement index
      stmt_infos.infos.iter_mut().for_each(|stmt_info| {
        stmt_info.import_records.iter().for_each(|rec_id| {
          let rec = &importer.import_records[*rec_id];
          if rec.is_dummy() {
            return;
          }

          match &self.module_table[rec.state] {
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
            Module::Normal(_) => {}
          }
        });
      });
    });
  }
}
