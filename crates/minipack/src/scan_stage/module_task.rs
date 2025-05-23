use std::{path::Path, sync::Arc};

use minipack_common::{
  Module, ModuleId, ModuleIdx, ModuleLoaderMsg, ModuleType, NormalModule, NormalModuleTaskResult,
  RUNTIME_MODULE_ID, ResolvedId,
};
use minipack_error::BuildResult;
use minipack_fs::{FileSystem, OsFileSystem};
use minipack_utils::{path_ext::PathExt, rstr::Rstr};
use oxc_index::IndexVec;
use tokio::sync::mpsc::Sender;

use crate::{
  types::{SharedOptions, SharedResolver},
  utils::ecmascript::legitimize_identifier_name,
};

use super::loaders::ecmascript::{CreateEcmaViewReturn, CreateModuleContext, create_ecma_view};

pub struct TaskContext {
  pub fs: OsFileSystem,
  pub options: SharedOptions,
  pub resolver: SharedResolver,
  pub tx: Sender<ModuleLoaderMsg>,
}

pub struct ModuleTask {
  ctx: Arc<TaskContext>,
  idx: ModuleIdx,
  owner: Option<Rstr>,
  resolved_id: ResolvedId,
  is_user_defined_entry: bool,
}

impl ModuleTask {
  pub fn new(
    ctx: Arc<TaskContext>,
    idx: ModuleIdx,
    owner: Option<Rstr>,
    resolved_id: ResolvedId,
    is_user_defined_entry: bool,
  ) -> Self {
    Self { ctx, idx, owner, resolved_id, is_user_defined_entry }
  }

  pub async fn run(mut self) {
    if let Err(errs) = self.run_inner().await {
      self.ctx.tx.send(ModuleLoaderMsg::BuildErrors(errs.0)).await.expect("Failed to send.");
    }
  }

  async fn run_inner(&mut self) -> BuildResult<()> {
    let (source, module_type) = self.load_source().map_err(|err| {
      anyhow::anyhow!(
        "Could not load {}{} - {}.",
        self.resolved_id.id,
        self.owner.as_ref().map(|owner| format!(" (imported by {})", owner)).unwrap_or_default(),
        err,
      )
    })?;

    let mut warnings = vec![];

    let id = ModuleId::new(&self.resolved_id.id);
    let stable_id = id.stabilize(&self.ctx.options.cwd);
    let repr_name = Path::new(self.resolved_id.id.as_str()).representative_file_name();
    let repr_name = legitimize_identifier_name(&repr_name).into_owned();
    let CreateEcmaViewReturn { ecma_view, ecma_related, raw_import_records } = create_ecma_view(
      &mut CreateModuleContext {
        module_idx: self.idx,
        repr_name: &repr_name,
        stable_id: &stable_id,
        module_type: module_type.clone(),
        warnings: &mut warnings,
      },
      source,
    )
    .await?;

    let resolved_deps = raw_import_records
      .iter()
      .map(|import_record| {
        let specifier = import_record.specifier.as_str();
        if specifier == RUNTIME_MODULE_ID {
          return Ok(ResolvedId { id: specifier.into(), is_external: false });
        }
        self.ctx.resolver.resolve_id(specifier, Some(&self.resolved_id.id), false)
      })
      .collect::<BuildResult<IndexVec<_, _>>>()?;

    let result = ModuleLoaderMsg::NormalModuleDone(Box::new(NormalModuleTaskResult {
      module: Module::Normal(Box::new(NormalModule {
        id,
        idx: self.idx,
        stable_id,
        ecma_view,
        exec_order: u32::MAX,
        module_type: module_type.clone(),
        is_user_defined_entry: self.is_user_defined_entry,
      })),
      ecma_related: Some(ecma_related),
      resolved_deps,
      raw_import_records,
      warnings,
    }));

    let _ = self.ctx.tx.send(result).await;

    Ok(())
  }

  pub fn load_source(&self) -> anyhow::Result<(String, ModuleType)> {
    let fs: &dyn FileSystem = &self.ctx.fs;
    let id = &self.resolved_id.id;

    let content = fs.read_to_string(Path::new(id.as_str()))?;
    let final_type = match id.rsplit('.').next().filter(|ext| ext != id) {
      Some("js" | "cjs" | "mjs") => ModuleType::Js,
      Some("ts" | "cts" | "mts") => ModuleType::Ts,
      _ => ModuleType::Js,
    };

    Ok((content, final_type))
  }
}
