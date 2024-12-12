use arcstr::ArcStr;
use minipack_common::{
  ImportKind, ImportRecordIdx, ModuleIdx, ModuleLoaderMsg, ModuleType, RawImportRecord, ResolvedId,
};
use minipack_resolver::ResolveError;
use minipack_utils::rstr::Rstr;
use oxc::span::Span;
use oxc_index::IndexVec;
use std::sync::Arc;

use crate::types::{SharedOptions, SharedResolver};

use super::task_context::TaskContext;

pub struct ModuleTaskOwner {
  source: ArcStr,
  importer_id: Rstr,
  importee_span: Span,
}

impl ModuleTaskOwner {
  pub fn new(source: ArcStr, importer_id: Rstr, importee_span: Span) -> Self {
    ModuleTaskOwner { source, importer_id, importee_span }
  }
}

pub struct ModuleTask {
  ctx: Arc<TaskContext>,
  module_idx: ModuleIdx,
  resolved_id: ResolvedId,
  owner: Option<ModuleTaskOwner>,
  errors: Vec<anyhow::Error>,
  is_user_defined_entry: bool,
  /// The module is asserted to be this specific module type.
  asserted_module_type: Option<ModuleType>,
}

impl ModuleTask {
  pub fn new(
    ctx: Arc<TaskContext>,
    idx: ModuleIdx,
    resolved_id: ResolvedId,
    owner: Option<ModuleTaskOwner>,
    is_user_defined_entry: bool,
    assert_module_type: Option<ModuleType>,
  ) -> Self {
    Self {
      ctx,
      module_idx: idx,
      resolved_id,
      owner,
      errors: vec![],
      is_user_defined_entry,
      asserted_module_type: assert_module_type,
    }
  }

  pub async fn run(mut self) {
    match self.run_inner().await {
      Ok(()) => {
        if !self.errors.is_empty() {
          self
            .ctx
            .tx
            .send(ModuleLoaderMsg::BuildErrors(self.errors))
            .await
            .expect("Send should not fail");
        }
      }
      Err(errs) => {
        self
          .ctx
          .tx
          .send(ModuleLoaderMsg::BuildErrors(vec![errs.into()]))
          .await
          .expect("Send should not fail");
      }
    }
  }

  async fn run_inner(&mut self) -> anyhow::Result<()> {
    todo!()
  }

  pub(crate) async fn resolve_id(
    bundle_options: &SharedOptions,
    resolver: &SharedResolver,
    importer: &str,
    specifier: &str,
    kind: ImportKind,
  ) -> anyhow::Result<anyhow::Result<ResolvedId, ResolveError>> {
    todo!()
  }

  pub async fn resolve_dependencies(
    &mut self,
    dependencies: &IndexVec<ImportRecordIdx, RawImportRecord>,
    source: ArcStr,
    warnings: &mut Vec<anyhow::Error>,
    module_type: &ModuleType,
  ) -> anyhow::Result<anyhow::Result<IndexVec<ImportRecordIdx, ResolvedId>>> {
    todo!()
  }
}
