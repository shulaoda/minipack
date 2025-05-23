use std::fmt::Debug;

use crate::ecmascript::ecma_view::EcmaView;
use crate::{EcmaAstIdx, Module, ModuleType};
use crate::{ImportRecordIdx, ImportRecordMeta, ModuleId, ModuleIdx};
use std::ops::{Deref, DerefMut};

use oxc_index::IndexVec;

#[derive(Debug)]
pub struct NormalModule {
  pub id: ModuleId,
  pub idx: ModuleIdx,
  pub exec_order: u32,
  pub stable_id: String,
  pub ecma_view: EcmaView,
  pub module_type: ModuleType,
  pub is_user_defined_entry: bool,
}

impl NormalModule {
  pub fn star_export_module_ids(&self) -> impl Iterator<Item = ModuleIdx> + '_ {
    if self.has_star_export() {
      itertools::Either::Left(
        self
          .ecma_view
          .import_records
          .iter()
          .filter(|&rec| rec.meta.contains(ImportRecordMeta::IS_EXPORT_STAR))
          .map(|rec| rec.state),
      )
    } else {
      itertools::Either::Right(std::iter::empty())
    }
  }

  pub fn has_star_export(&self) -> bool {
    self.ecma_view.meta.has_star_export()
  }

  pub fn ecma_ast_idx(&self) -> EcmaAstIdx {
    self.ecma_view.ecma_ast_idx.expect("ecma_ast_idx should be set in this stage")
  }

  pub fn star_exports_from_external_modules<'me>(
    &'me self,
    modules: &'me IndexVec<ModuleIdx, Module>,
  ) -> impl Iterator<Item = ImportRecordIdx> + 'me {
    self.ecma_view.import_records.iter_enumerated().filter_map(move |(rec_id, rec)| {
      if !rec.meta.contains(ImportRecordMeta::IS_EXPORT_STAR) {
        return None;
      }
      match modules[rec.state] {
        Module::External(_) => Some(rec_id),
        Module::Normal(_) => None,
      }
    })
  }

  pub fn is_included(&self) -> bool {
    self.ecma_view.meta.is_included()
  }
}

impl Deref for NormalModule {
  type Target = EcmaView;

  fn deref(&self) -> &Self::Target {
    &self.ecma_view
  }
}

impl DerefMut for NormalModule {
  fn deref_mut(&mut self) -> &mut Self::Target {
    &mut self.ecma_view
  }
}
