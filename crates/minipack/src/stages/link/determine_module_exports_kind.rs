use std::ptr::addr_of;

use minipack_common::{ExportsKind, ImportKind, Module, OutputFormat, WrapKind};
use rustc_hash::FxHashSet;

use super::LinkStage;

impl LinkStage<'_> {
  pub(crate) fn determine_module_exports_kind(&mut self) {
    let entry_ids_set = self.entry_points.iter().map(|e| e.id).collect::<FxHashSet<_>>();
    self.module_table.modules.iter().filter_map(Module::as_normal).for_each(|importer| {
      // TODO(hyf0): should check if importer is a js module
      importer.import_records.iter().for_each(|rec| {
        let importee_id = rec.resolved_module;
        let Module::Normal(importee) = &self.module_table.modules[importee_id] else {
          return;
        };

        match rec.kind {
          ImportKind::Import => {
            if matches!(importee.exports_kind, ExportsKind::None)
              && !importee.meta.has_lazy_export()
            {
              // `import` a module that has `ExportsKind::None`, which will be turned into `ExportsKind::Esm`
              // SAFETY: If `importee` and `importer` are different, so this is safe. If they are the same, then behaviors are still expected.
              unsafe {
                let importee_mut = addr_of!(*importee).cast_mut();
                (*importee_mut).exports_kind = ExportsKind::Esm;
              }
            }
          }
          ImportKind::Require => match importee.exports_kind {
            ExportsKind::Esm => {
              self.metadata[importee.idx].wrap_kind = WrapKind::Esm;
            }
            ExportsKind::CommonJs => {
              self.metadata[importee.idx].wrap_kind = WrapKind::Cjs;
            }
            ExportsKind::None => {
              self.metadata[importee.idx].wrap_kind = WrapKind::Cjs;
              // SAFETY: If `importee` and `importer` are different, so this is safe. If they are the same, then behaviors are still expected.
              // A module with `ExportsKind::None` that `require` self should be turned into `ExportsKind::CommonJs`.
              unsafe {
                let importee_mut = addr_of!(*importee).cast_mut();
                (*importee_mut).exports_kind = ExportsKind::CommonJs;
              }
            }
          },
          ImportKind::DynamicImport => {
            if self.options.inline_dynamic_imports {
              // For iife, then import() is just a require() that
              // returns a promise, so the imported file must also be wrapped
              match importee.exports_kind {
                ExportsKind::Esm => {
                  self.metadata[importee.idx].wrap_kind = WrapKind::Esm;
                }
                ExportsKind::CommonJs => {
                  self.metadata[importee.idx].wrap_kind = WrapKind::Cjs;
                }
                ExportsKind::None => {
                  self.metadata[importee.idx].wrap_kind = WrapKind::Cjs;
                  // SAFETY: If `importee` and `importer` are different, so this is safe. If they are the same, then behaviors are still expected.
                  // A module with `ExportsKind::None` that `require` self should be turned into `ExportsKind::CommonJs`.
                  unsafe {
                    let importee_mut = addr_of!(*importee).cast_mut();
                    (*importee_mut).exports_kind = ExportsKind::CommonJs;
                  }
                }
              }
            }
          }
          ImportKind::AtImport => {
            unreachable!("A Js module would never import a CSS module via `@import`");
          }
          ImportKind::UrlImport => {
            unreachable!("A Js module would never import a CSS module via `url()`");
          }
          ImportKind::NewUrl => {}
        }
      });

      let is_entry = entry_ids_set.contains(&importer.idx);
      if matches!(importer.exports_kind, ExportsKind::CommonJs)
        && (!is_entry || matches!(self.options.format, OutputFormat::Esm))
      {
        self.metadata[importer.idx].wrap_kind = WrapKind::Cjs;
      }
    });
  }
}
