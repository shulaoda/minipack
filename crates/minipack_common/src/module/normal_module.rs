use std::fmt::Debug;
use std::sync::Arc;

use crate::ecmascript::ecma_view::EcmaView;
use crate::{ImportRecordIdx, ImportRecordMeta, ModuleId, ModuleIdx};
use crate::{EcmaAstIdx, Module, ModuleType};
use std::ops::{Deref, DerefMut};

use minipack_ecmascript::{EcmaAst, EcmaCompiler};
use oxc_index::IndexVec;

#[derive(Debug)]
pub struct NormalModule {
  pub exec_order: u32,
  pub idx: ModuleIdx,
  pub is_user_defined_entry: bool,
  pub id: ModuleId,
  /// `stable_id` is calculated based on `id` to be stable across machine and os.
  pub stable_id: String,
  // Pretty resource id for debug
  pub debug_id: String,
  pub repr_name: String,
  pub module_type: ModuleType,
  pub ecma_view: EcmaView,
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
          .map(|rec| rec.resolved_module),
      )
    } else {
      itertools::Either::Right(std::iter::empty())
    }
  }

  pub fn has_star_export(&self) -> bool {
    self.ecma_view.meta.has_star_export()
  }

  pub fn is_js_type(&self) -> bool {
    matches!(self.module_type, ModuleType::Js | ModuleType::Jsx | ModuleType::Ts | ModuleType::Tsx)
  }

  pub fn ecma_ast_idx(&self) -> EcmaAstIdx {
    self.ecma_view.ecma_ast_idx.expect("ecma_ast_idx should be set in this stage")
  }

  pub fn star_exports_from_external_modules<'me>(
    &'me self,
    modules: &'me IndexVec<ModuleIdx, Module>,
  ) -> impl Iterator<Item = ImportRecordIdx> + 'me {
    self.ecma_view.import_records.iter_enumerated().filter_map(move |(rec_id, rec)| {
      if !rec.meta.contains(ImportRecordMeta::IS_EXPORT_STAR)
        || rec.meta.contains(ImportRecordMeta::IS_DUMMY)
      {
        return None;
      }
      match modules[rec.resolved_module] {
        Module::External(_) => Some(rec_id),
        Module::Normal(_) => None,
      }
    })
  }

  pub fn render(&self, ast: &EcmaAst) -> Option<String> {
    let render_output = EcmaCompiler::print(ast);
    if !self.ecma_view.mutations.is_empty() {
      let original_code: Arc<str> = render_output.code.into();
      let magic_string = string_wizard::MagicString::new(&*original_code);
      let code = magic_string.to_string();
      return Some(code);
    }
    Some(render_output.code)
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
