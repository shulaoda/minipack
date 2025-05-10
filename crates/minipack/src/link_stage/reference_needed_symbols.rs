use std::{ptr::addr_of, sync::Mutex};

use minipack_common::{ImportRecordMeta, Module, OutputFormat};
use minipack_utils::{
  concat_string,
  rayon::{IntoParallelRefIterator, ParallelIterator},
};

use crate::utils::ecmascript::legitimize_identifier_name;

use super::LinkStage;

impl LinkStage {
  pub(crate) fn reference_needed_symbols(&mut self) {
    let symbol_ref_db = Mutex::new(&mut self.symbol_ref_db);
    self.module_table.par_iter().filter_map(|m| m.as_normal()).for_each(|normal_module| {
      // safety: No race conditions here:
      // - Mutating on `stmt_infos` is isolated in threads for each module
      // - Mutating on `stmt_infos` doesn't rely on other mutating operations of other modules
      // - Mutating and parallel reading is in different memory locations
      let stmt_infos = unsafe { &mut *(addr_of!(normal_module.stmt_infos).cast_mut()) };

      stmt_infos.infos.iter_mut().for_each(|stmt_info| {
        stmt_info.import_records.iter().for_each(|&import_record_id| {
          let import_record = &normal_module.import_records[import_record_id];

          if import_record.kind.is_static() {
            return;
          }

          if let Module::External(importee) = &self.module_table[import_record.state] {
            // Make sure symbols from external modules are included and de_conflicted
            if import_record.meta.contains(ImportRecordMeta::IS_EXPORT_STAR) {
              // export * from 'external' would be just removed. So it references nothing.
              import_record.namespace_ref.set_name(
                &mut symbol_ref_db.lock().unwrap(),
                &concat_string!("import_", legitimize_identifier_name(&importee.name)),
              );
            } else {
              // import ... from 'external' or export ... from 'external'
              if !import_record.meta.contains(ImportRecordMeta::IS_PLAIN_IMPORT)
                && matches!(self.options.format, OutputFormat::Cjs)
              {
                stmt_info.side_effect = true;
                stmt_info
                  .referenced_symbols
                  .push(self.runtime_module.resolve_symbol("__toESM").into());
              }
            }
          }
        });
      });
    });
  }
}
