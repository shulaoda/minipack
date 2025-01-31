use arcstr::ArcStr;
use minipack_common::{
  ImportKind, ImportRecordIdx, ImportRecordMeta, ModuleDefFormat, ModuleId, ModuleIdx,
  ModuleLoaderMsg, ModuleType, NormalModule, NormalModuleTaskResult, ResolvedId, StrOrBytes,
  RUNTIME_MODULE_ID,
};
use minipack_error::BuildResult;
use minipack_fs::FileSystem;
use minipack_utils::{ecmascript::legitimize_identifier_name, path_ext::PathExt, rstr::Rstr};
use oxc_index::IndexVec;
use std::sync::Arc;
use sugar_path::SugarPath;

use crate::{
  module_loader::loaders::{
    asset::create_asset_view,
    css::create_css_view,
    ecmascript::ecma_module_view_factory::{create_ecma_view, CreateEcmaViewReturn},
  },
  types::module_factory::{CreateModuleContext, CreateModuleViewArgs},
  utils::resolve_id::resolve_id,
};

use super::task_context::TaskContext;

pub struct ModuleTaskOwner {
  importer_id: Rstr,
}

impl ModuleTaskOwner {
  pub fn new(importer_id: Rstr) -> Self {
    ModuleTaskOwner { importer_id }
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
    asserted_module_type: Option<ModuleType>,
  ) -> Self {
    Self { ctx, idx, owner, resolved_id, is_user_defined_entry, asserted_module_type }
  }

  pub async fn run(mut self) {
    if let Err(errs) = self.run_inner().await {
      self.ctx.tx.send(ModuleLoaderMsg::BuildErrors(errs.0)).await.expect("Send should not fail");
    }
  }

  async fn run_inner(&mut self) -> BuildResult<()> {
    let (mut source, module_type) = self.load_source().map_err(|err| {
      anyhow::anyhow!(
        "Could not load {}{} - {}.",
        self.resolved_id.debug_id(self.ctx.options.cwd.as_path()),
        self
          .owner
          .as_ref()
          .map(|owner| format!(" (imported by {})", owner.importer_id))
          .unwrap_or_default(),
        err,
      )
    })?;

    let mut css_view = None;
    let mut asset_view = None;
    let mut raw_import_records = IndexVec::default();

    match module_type {
      ModuleType::Asset => {
        let asset_source = source.into_bytes();
        asset_view = Some(create_asset_view(asset_source.into()));
        source = StrOrBytes::default();
      }
      ModuleType::Css => {
        let css_source = source.try_into_string()?.into();
        // FIXME: This makes creating `EcmaView` rely on creating `CssView` first, while they should be done in parallel.
        let (raw_css_view, import_records) = create_css_view(css_source);
        raw_import_records = import_records;
        css_view = Some(raw_css_view);
        source = StrOrBytes::default();
      }
      _ => {}
    };

    let mut warnings = vec![];

    let hook_side_effects = self.resolved_id.side_effects.take();

    let ret = create_ecma_view(
      &mut CreateModuleContext {
        module_index: self.idx,
        resolved_id: &self.resolved_id,
        options: &self.ctx.options,
        warnings: &mut warnings,
        module_type: module_type.clone(),
        is_user_defined_entry: self.is_user_defined_entry,
      },
      CreateModuleViewArgs { source, hook_side_effects },
    )
    .await?;

    let CreateEcmaViewReturn {
      view: mut ecma_view,
      ecma_related,
      raw_import_records: ecma_raw_import_records,
    } = ret;

    if css_view.is_none() {
      raw_import_records = ecma_raw_import_records;
    }

    let resolved_deps = raw_import_records
      .iter()
      .map(|item| {
        if item.meta.contains(ImportRecordMeta::IS_DUMMY) {
          return Ok(ResolvedId::make_dummy());
        }

        let specifier = item.module_request.as_str();

        // Check runtime module
        if specifier == RUNTIME_MODULE_ID {
          return Ok(ResolvedId {
            id: item.module_request.to_string().into(),
            ignored: false,
            module_def_format: ModuleDefFormat::EsmMjs,
            is_external: false,
            package_json: None,
            side_effects: None,
            is_external_without_side_effects: false,
          });
        }

        resolve_id(&self.ctx.resolver, specifier, Some(&self.resolved_id.id), item.kind, false)
      })
      .collect::<BuildResult<IndexVec<ImportRecordIdx, ResolvedId>>>()?;

    if css_view.is_none() {
      for (record, info) in raw_import_records.iter().zip(&resolved_deps) {
        match record.kind {
          ImportKind::Import | ImportKind::Require | ImportKind::NewUrl => {
            ecma_view.imported_ids.insert(ArcStr::clone(&info.id).into());
          }
          ImportKind::DynamicImport => {
            ecma_view.dynamically_imported_ids.insert(ArcStr::clone(&info.id).into());
          }
          // for a none css module, we should not have `at-import` or `url-import`
          ImportKind::AtImport | ImportKind::UrlImport => unreachable!(),
        }
      }
    }

    let repr_name = self.resolved_id.id.as_path().representative_file_name().into_owned();
    let repr_name = legitimize_identifier_name(&repr_name).into_owned();

    let id = ModuleId::new(&self.resolved_id.id);
    let stable_id = id.stabilize(&self.ctx.options.cwd);
    let debug_id = self.resolved_id.debug_id(&self.ctx.options.cwd);

    let module = NormalModule {
      id,
      idx: self.idx,
      debug_id,
      stable_id,
      repr_name,
      exec_order: u32::MAX,
      ecma_view,
      css_view,
      asset_view,
      module_type: module_type.clone(),
      is_user_defined_entry: self.is_user_defined_entry,
    };

    let result = ModuleLoaderMsg::NormalModuleDone(NormalModuleTaskResult {
      module: module.into(),
      resolved_deps,
      ecma_related: Some(ecma_related),
      raw_import_records,
      warnings,
    });

    let _ = self.ctx.tx.send(result).await;

    Ok(())
  }

  pub fn load_source(&self) -> anyhow::Result<(StrOrBytes, ModuleType)> {
    let fs: &dyn FileSystem = &self.ctx.fs;

    if self.resolved_id.ignored {
      return Ok((
        StrOrBytes::default(),
        self.asserted_module_type.clone().unwrap_or(ModuleType::Empty),
      ));
    }

    let id = &self.resolved_id.id;
    let ext = id.rsplit('.').next().filter(|ext| *ext != id).unwrap_or("");

    let module_type = match ext {
      "js" | "mjs" | "cjs" => ModuleType::Js,
      "ts" | "mts" | "cts" => ModuleType::Ts,
      "jsx" => ModuleType::Jsx,
      "tsx" => ModuleType::Tsx,
      "json" => ModuleType::Json,
      "txt" => ModuleType::Text,
      "css" => ModuleType::Css,
      _ => ModuleType::Js,
    };

    let content = match module_type {
      ModuleType::Base64 | ModuleType::Binary | ModuleType::Dataurl | ModuleType::Asset => {
        StrOrBytes::Bytes(fs.read(id.as_path())?)
      }
      _ => StrOrBytes::Str(fs.read_to_string(id.as_path())?),
    };

    let final_type = self.asserted_module_type.clone().unwrap_or(module_type);

    Ok((content, final_type))
  }
}
