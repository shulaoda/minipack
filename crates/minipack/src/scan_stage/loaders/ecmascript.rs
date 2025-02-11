use minipack_common::{
  side_effects::DeterminedSideEffects, EcmaRelated, EcmaView, EcmaViewMeta, ImportRecordIdx,
  ModuleId, RawImportRecord, StrOrBytes,
};
use minipack_error::BuildResult;
use minipack_utils::{
  ecmascript::legitimize_identifier_name, indexmap::FxIndexSet, path_ext::PathExt,
};
use oxc_index::IndexVec;

use sugar_path::SugarPath;

use crate::{
  scan_stage::ast_scanner::{AstScanResult, AstScanner},
  types::module_factory::CreateModuleContext,
  utils::parse_to_ecma_ast::{parse_to_ecma_ast, ParseToEcmaAstResult},
};

pub struct CreateEcmaViewReturn {
  pub ecma_view: EcmaView,
  pub ecma_related: EcmaRelated,
  pub raw_import_records: IndexVec<ImportRecordIdx, RawImportRecord>,
}

pub async fn create_ecma_view(
  ctx: &mut CreateModuleContext<'_>,
  source: StrOrBytes,
) -> BuildResult<CreateEcmaViewReturn> {
  let ParseToEcmaAstResult { ast, symbols, scopes, has_lazy_export, warning } =
    parse_to_ecma_ast(ctx, source)?;

  ctx.warnings.extend(warning);

  let module_id = ModuleId::new(&ctx.resolved_id.id);
  let repr_name = module_id.as_path().representative_file_name();
  let repr_name = legitimize_identifier_name(&repr_name);

  let scanner = AstScanner::new(
    ctx.module_idx,
    scopes,
    symbols,
    &repr_name,
    ctx.resolved_id.module_def_format,
    ast.source(),
    &module_id,
    ast.comments(),
    ctx.options,
  );

  let AstScanResult {
    named_imports,
    named_exports,
    stmt_infos,
    import_records: raw_import_records,
    default_export_ref,
    imports,
    exports_kind,
    namespace_object_ref,
    warnings: scan_warnings,
    has_eval,
    errors,
    ast_usage,
    scopes,
    symbols,
    self_referenced_class_decl_symbol_ids,
    hashbang_range,
    has_star_exports,
    dynamic_import_rec_exports_usage: dynamic_import_exports_usage,
    new_url_references: new_url_imports,
    this_expr_replace_map,
  } = scanner.scan(ast.program())?;

  if !errors.is_empty() {
    return Err(errors.into());
  }

  ctx.warnings.extend(scan_warnings);

  let ecma_view = EcmaView {
    source: ast.source().clone(),
    ecma_ast_idx: None,
    named_imports,
    named_exports,
    stmt_infos,
    imports,
    default_export_ref,
    ast_scope_idx: None,
    exports_kind,
    namespace_object_ref,
    def_format: ctx.resolved_id.module_def_format,
    import_records: IndexVec::default(),
    importers: FxIndexSet::default(),
    dynamic_importers: FxIndexSet::default(),
    imported_ids: FxIndexSet::default(),
    dynamically_imported_ids: FxIndexSet::default(),
    side_effects: DeterminedSideEffects::NoTreeshake,
    ast_usage,
    self_referenced_class_decl_symbol_ids,
    hashbang_range,
    meta: {
      let mut meta = EcmaViewMeta::default();
      meta.set(EcmaViewMeta::EVAL, has_eval);
      meta.set(EcmaViewMeta::HAS_LAZY_EXPORT, has_lazy_export);
      meta.set(EcmaViewMeta::HAS_STAR_EXPORT, has_star_exports);
      meta
    },
    mutations: vec![],
    new_url_references: new_url_imports,
    this_expr_replace_map,
    esm_namespace_in_cjs: None,
    esm_namespace_in_cjs_node_mode: None,
  };

  let ecma_related = EcmaRelated { ast, scopes, symbols, dynamic_import_exports_usage };
  Ok(CreateEcmaViewReturn { ecma_view, ecma_related, raw_import_records })
}
