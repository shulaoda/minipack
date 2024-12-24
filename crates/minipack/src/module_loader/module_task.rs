use arcstr::ArcStr;
use minipack_common::{
  EcmaRelated, ImportKind, ImportRecordIdx, ModuleDefFormat, ModuleId, ModuleIdx, ModuleLoaderMsg,
  ModuleType, NormalModule, NormalModuleTaskResult, ResolvedId, StrOrBytes, RUNTIME_MODULE_ID,
};
use minipack_error::BuildResult;
use minipack_utils::{ecmascript::legitimize_identifier_name, path_ext::PathExt, rstr::Rstr};
use oxc::span::Span;
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
  utils::{load_source::load_source, resolve_id::resolve_id},
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
      self.ctx.tx.send(ModuleLoaderMsg::BuildErrors(errs.0)).await.expect("Send should not fail");
    }
  }

  async fn run_inner(&mut self) -> BuildResult<()> {
    let id = ModuleId::new(ArcStr::clone(&self.resolved_id.id));

    let hook_side_effects = self.resolved_id.side_effects.take();

    let result = load_source(&self.ctx.fs, &self.resolved_id);

    let (mut source, mut module_type) = result.map_err(|err| {
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

    if let Some(asserted) = &self.asserted_module_type {
      module_type = asserted.clone();
    }

    let asset_view = if matches!(module_type, ModuleType::Asset) {
      let asset_source = source.into_bytes();
      source = StrOrBytes::Str(String::new());
      Some(create_asset_view(asset_source.into()))
    } else {
      None
    };

    let stable_id = id.stabilize(&self.ctx.options.cwd);
    let mut raw_import_records = IndexVec::default();

    let css_view = if matches!(module_type, ModuleType::Css) {
      let css_source: ArcStr = source.try_into_string()?.into();
      // FIXME: This makes creating `EcmaView` rely on creating `CssView` first, while they should be done in parallel.
      source = StrOrBytes::Str(String::new());
      let create_ret = create_css_view(&stable_id, &css_source);
      raw_import_records = create_ret.1;
      Some(create_ret.0)
    } else {
      None
    };

    let mut warnings = vec![];

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
      ast,
      symbols,
      raw_import_records: ecma_raw_import_records,
      dynamic_import_rec_exports_usage,
    } = ret;

    if !matches!(module_type, ModuleType::Css) {
      raw_import_records = ecma_raw_import_records;
    }

    let resolved_deps = raw_import_records
      .iter()
      .map(|item| self.resolve_id(&item.module_request, item.kind))
      .collect::<BuildResult<IndexVec<ImportRecordIdx, ResolvedId>>>()?;

    if !matches!(module_type, ModuleType::Css) {
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
    let repr_name = legitimize_identifier_name(&repr_name);

    let module = NormalModule {
      repr_name: repr_name.into_owned(),
      stable_id,
      id,
      debug_id: self.resolved_id.debug_id(&self.ctx.options.cwd),
      idx: self.idx,
      exec_order: u32::MAX,
      is_user_defined_entry: self.is_user_defined_entry,
      module_type: module_type.clone(),
      ecma_view,
      css_view,
      asset_view,
    };

    let result = ModuleLoaderMsg::NormalModuleDone(NormalModuleTaskResult {
      resolved_deps,
      module_idx: self.idx,
      warnings,
      ecma_related: Some(EcmaRelated { ast, symbols, dynamic_import_rec_exports_usage }),
      module: module.into(),
      raw_import_records,
    });

    let _ = self.ctx.tx.send(result).await;

    Ok(())
  }

  fn resolve_id(&self, specifier: &str, kind: ImportKind) -> BuildResult<ResolvedId> {
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

    let importer = &self.resolved_id.id;
    let resolver = &self.ctx.resolver;

    resolve_id(resolver, specifier, Some(importer), kind, false)
  }
}
