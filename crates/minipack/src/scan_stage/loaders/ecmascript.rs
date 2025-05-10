use minipack_common::{
  EcmaRelated, EcmaView, EcmaViewMeta, ImportRecordIdx, ModuleIdx, ModuleType, RawImportRecord,
  side_effects::DeterminedSideEffects,
};
use minipack_error::BuildResult;
use oxc_index::IndexVec;

use sugar_path::SugarPath;

use crate::{
  scan_stage::ast_scanner::{AstScanResult, AstScanner},
  utils::parse_to_ecma_ast,
};

pub struct CreateModuleContext<'a> {
  pub stable_id: &'a str,
  pub repr_name: &'a str,
  pub module_idx: ModuleIdx,
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
  let (ast, scoping) = parse_to_ecma_ast(source, ctx.stable_id.as_path(), &ctx.module_type)?;

  let AstScanResult {
    named_imports,
    named_exports,
    stmt_infos,
    import_records: raw_import_records,
    default_export_ref,
    imports,
    namespace_object_ref,
    warnings,
    errors,
    symbols,
    has_star_exports,
  } = AstScanner::new(ctx.module_idx, scoping, ctx.repr_name).scan(ast.program())?;

  if !errors.is_empty() {
    Err(errors)?;
  }

  let has_side_effects = stmt_infos.iter().any(|stmt_info| stmt_info.side_effect);
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
    side_effects: DeterminedSideEffects::Analyzed(has_side_effects),
    meta: {
      let mut meta = EcmaViewMeta::default();
      meta.set(EcmaViewMeta::HAS_STAR_EXPORT, has_star_exports);
      meta
    },
  };

  ctx.warnings.extend(warnings);
  Ok(CreateEcmaViewReturn {
    ecma_view,
    ecma_related: EcmaRelated { ast, symbols },
    raw_import_records,
  })
}
