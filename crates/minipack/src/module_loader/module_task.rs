use arcstr::ArcStr;
use minipack_common::{
  ImportKind, ImportRecordIdx, ModuleDefFormat, ModuleIdx, ModuleLoaderMsg, ModuleType,
  RawImportRecord, ResolvedId, RUNTIME_MODULE_ID,
};
use minipack_error::BuildResult;
use minipack_utils::rstr::Rstr;
use oxc::span::Span;
use oxc_index::IndexVec;
use std::sync::Arc;

use crate::{
  types::{SharedOptions, SharedResolver},
  utils::resolve_id::resolve_id,
};

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
  idx: ModuleIdx,
  owner: Option<ModuleTaskOwner>,
  resolved_id: ResolvedId,
  is_user_defined_entry: bool,
  /// The module is asserted to be this specific module type.
  asserted_module_type: Option<ModuleType>,
}

impl ModuleTask {
  pub fn new(
    ctx: Arc<TaskContext>,
    idx: ModuleIdx,
    owner: Option<ModuleTaskOwner>,
    resolved_id: ResolvedId,
    is_user_defined_entry: bool,
    assert_module_type: Option<ModuleType>,
  ) -> Self {
    Self {
      ctx,
      idx,
      owner,
      resolved_id,
      is_user_defined_entry,
      asserted_module_type: assert_module_type,
    }
  }

  pub async fn run(mut self) {
    if let Err(errs) = self.run_inner().await {
      self.ctx.tx.send(ModuleLoaderMsg::BuildErrors(errs)).await.expect("Send should not fail");
    }
  }

  async fn run_inner(&mut self) -> BuildResult<()> {
    todo!()
  }

  pub(crate) async fn resolve_id(
    bundle_options: &SharedOptions,
    resolver: &SharedResolver,
    importer: &str,
    specifier: &str,
    kind: ImportKind,
  ) -> BuildResult<ResolvedId> {
    // Check runtime module
    if specifier == RUNTIME_MODULE_ID {
      return Ok(ResolvedId {
        id: specifier.to_string().into(),
        ignored: false,
        module_def_format: ModuleDefFormat::EsmMjs,
        is_external: false,
        package_json: None,
        side_effects: None,
        is_external_without_side_effects: false,
      });
    }

    resolve_id(resolver, specifier, Some(importer), kind, false)
  }

  pub async fn resolve_dependencies(
    &mut self,
    dependencies: &IndexVec<ImportRecordIdx, RawImportRecord>,
    source: ArcStr,
    warnings: &mut Vec<anyhow::Error>,
    module_type: &ModuleType,
  ) -> BuildResult<IndexVec<ImportRecordIdx, ResolvedId>> {
    todo!()
  }
}
