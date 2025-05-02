use std::{path::Path, sync::Arc};

use arcstr::ArcStr;
use minipack_common::{
  ImportKind, ImportRecordIdx, ImportRecordMeta, Module, ModuleId, ModuleIdx, ModuleLoaderMsg,
  ModuleType, NormalModule, NormalModuleTaskResult, RUNTIME_MODULE_ID, ResolvedId,
};
use minipack_error::BuildResult;
use minipack_fs::FileSystem;
use minipack_utils::{path_ext::PathExt, rstr::Rstr};
use oxc_index::IndexVec;

use crate::{
  types::module_factory::CreateModuleContext,
  utils::{ecmascript::legitimize_identifier_name, resolve_id::resolve_id},
};

use super::loaders::ecmascript::{CreateEcmaViewReturn, create_ecma_view};
use super::task_context::TaskContext;

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
      self.ctx.tx.send(ModuleLoaderMsg::BuildErrors(errs.0)).await.expect("Send should not fail");
    }
  }

  async fn run_inner(&mut self) -> BuildResult<()> {
    let (source, module_type) = self.load_source().map_err(|err| {
      anyhow::anyhow!(
        "Could not load {}{} - {}.",
        self.resolved_id.debug_id(self.ctx.options.cwd.as_path()),
        self.owner.as_ref().map(|owner| format!(" (imported by {})", owner)).unwrap_or_default(),
        err,
      )
    })?;

    let mut warnings = vec![];

    let id = ModuleId::new(&self.resolved_id.id);
    let stable_id = id.stabilize(&self.ctx.options.cwd);
    let ret = create_ecma_view(
      &mut CreateModuleContext {
        stable_id: &stable_id,
        module_idx: self.idx,
        resolved_id: &self.resolved_id,
        options: &self.ctx.options,
        warnings: &mut warnings,
        module_type: module_type.clone(),
      },
      source,
    )
    .await?;

    let CreateEcmaViewReturn { mut ecma_view, ecma_related, raw_import_records } = ret;

    let resolved_deps = raw_import_records
      .iter()
      .map(|item| {
        if item.meta.contains(ImportRecordMeta::IS_DUMMY) {
          return Ok(ResolvedId::make_dummy());
        }

        let specifier = item.module_request.as_str();
        if specifier == RUNTIME_MODULE_ID {
          return Ok(ResolvedId {
            id: specifier.into(),
            ignored: false,
            is_external: false,
            package_json: None,
          });
        }

        resolve_id(&self.ctx.resolver, specifier, Some(&self.resolved_id.id), false)
      })
      .collect::<BuildResult<IndexVec<ImportRecordIdx, ResolvedId>>>()?;

    for (record, info) in raw_import_records
      .iter()
      .filter(|rec| !rec.meta.contains(ImportRecordMeta::IS_DUMMY))
      .zip(&resolved_deps)
    {
      match record.kind {
        ImportKind::Import => {
          ecma_view.imported_ids.insert(ArcStr::clone(&info.id).into());
        }
        ImportKind::DynamicImport => {
          ecma_view.dynamically_imported_ids.insert(ArcStr::clone(&info.id).into());
        }
      }
    }

    let repr_name = Path::new(self.resolved_id.id.as_str()).representative_file_name();
    let repr_name = legitimize_identifier_name(&repr_name).into_owned();

    let debug_id = self.resolved_id.debug_id(&self.ctx.options.cwd);

    let result = ModuleLoaderMsg::NormalModuleDone(NormalModuleTaskResult {
      module: Module::Normal(Box::new(NormalModule {
        id,
        idx: self.idx,
        debug_id,
        stable_id,
        repr_name,
        ecma_view,
        exec_order: u32::MAX,
        module_type: module_type.clone(),
        is_user_defined_entry: self.is_user_defined_entry,
      })),
      ecma_related: Some(ecma_related),
      resolved_deps,
      raw_import_records,
      warnings,
    });

    let _ = self.ctx.tx.send(result).await;

    Ok(())
  }

  pub fn load_source(&self) -> anyhow::Result<(String, ModuleType)> {
    let fs: &dyn FileSystem = &self.ctx.fs;

    if self.resolved_id.ignored {
      return Ok((String::new(), ModuleType::Empty));
    }

    let id = &self.resolved_id.id;
    let content = fs.read_to_string(Path::new(id.as_str()))?;

    let final_type = match id.rsplit('.').next().filter(|ext| *ext != id) {
      Some("js" | "cjs" | "mjs") => ModuleType::Js,
      Some("ts" | "cts" | "mts") => ModuleType::Ts,
      Some("jsx") => ModuleType::Jsx,
      Some("tsx") => ModuleType::Tsx,
      _ => ModuleType::Js,
    };

    Ok((content, final_type))
  }
}
