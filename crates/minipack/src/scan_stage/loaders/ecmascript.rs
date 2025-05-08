use minipack_common::{
  EcmaRelated, EcmaView, EcmaViewMeta, ImportRecordIdx, ModuleId, ModuleIdx, ModuleType,
  RawImportRecord, ResolvedId, side_effects::DeterminedSideEffects,
};
use minipack_error::BuildResult;
use minipack_utils::{indexmap::FxIndexSet, path_ext::PathExt};
use oxc_index::IndexVec;

use sugar_path::SugarPath;

use crate::{
  scan_stage::ast_scanner::{AstScanResult, AstScanner},
  utils::{
    ecmascript::legitimize_identifier_name,
    parse_to_ecma_ast::{ParseToEcmaAstResult, parse_to_ecma_ast},
  },
};

pub struct CreateModuleContext<'a> {
  pub stable_id: &'a str,
  pub module_idx: ModuleIdx,
  pub resolved_id: &'a ResolvedId,
  pub module_type: ModuleType,
  pub warnings: &'a mut Vec<anyhow::Error>,
}

pub struct CreateEcmaViewReturn {
  pub ecma_view: EcmaView,
  pub ecma_related: EcmaRelated,
  pub raw_import_records: IndexVec<ImportRecordIdx, RawImportRecord>,
}

pub async fn create_ecma_view(
  ctx: &mut CreateModuleContext<'_>,
  source: String,
) -> BuildResult<CreateEcmaViewReturn> {
  let ParseToEcmaAstResult { ast, scoping, warning } =
    parse_to_ecma_ast(ctx.stable_id.as_path(), &ctx.module_type, source)?;

  ctx.warnings.extend(warning);

  let module_id = ModuleId::new(&ctx.resolved_id.id);
  let repr_name = module_id.as_path().representative_file_name();
  let repr_name = legitimize_identifier_name(&repr_name);

  let scanner = AstScanner::new(ctx.module_idx, scoping, &repr_name, &module_id);

  let AstScanResult {
    named_imports,
    named_exports,
    stmt_infos,
    import_records: raw_import_records,
    default_export_ref,
    imports,
    namespace_object_ref,
    warnings: scan_warnings,
    errors,
    symbols,
    has_star_exports,
  } = scanner.scan(ast.program())?;

  if !errors.is_empty() {
    return Err(errors.into());
  }

  ctx.warnings.extend(scan_warnings);

  let side_effects =
    DeterminedSideEffects::Analyzed(stmt_infos.iter().any(|stmt_info| stmt_info.side_effect));

  let ecma_view = EcmaView {
    source: ast.source().clone(),
    ecma_ast_idx: None,
    named_imports,
    named_exports,
    stmt_infos,
    imports,
    default_export_ref,
    namespace_object_ref,
    import_records: IndexVec::default(),
    importers: FxIndexSet::default(),
    dynamic_importers: FxIndexSet::default(),
    imported_ids: FxIndexSet::default(),
    dynamically_imported_ids: FxIndexSet::default(),
    side_effects,
    meta: {
      let mut meta = EcmaViewMeta::default();
      meta.set(EcmaViewMeta::HAS_STAR_EXPORT, has_star_exports);
      meta
    },
  };

  Ok(CreateEcmaViewReturn {
    ecma_view,
    ecma_related: EcmaRelated { ast, symbols },
    raw_import_records,
  })
}
