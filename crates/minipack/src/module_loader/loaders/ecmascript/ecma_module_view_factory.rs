use arcstr::ArcStr;
use minipack_common::{
  dynamic_import_usage::DynamicImportExportsUsage,
  side_effects::{DeterminedSideEffects, HookSideEffects},
  AstScopes, EcmaView, EcmaViewMeta, ImportRecordIdx, ModuleDefFormat, ModuleId, ModuleIdx,
  ModuleType, RawImportRecord, SymbolRef, SymbolRefDbForModule,
};
use minipack_ecmascript::EcmaAst;
use minipack_error::BuildResult;
use minipack_utils::{
  ecmascript::legitimize_identifier_name, indexmap::FxIndexSet, path_ext::PathExt,
};
use oxc::semantic::{ScopeTree, SymbolTable};
use oxc_index::IndexVec;

use rustc_hash::FxHashMap;
use sugar_path::SugarPath;

use crate::{
  module_loader::ast_scanner::{AstScanResult, AstScanner},
  types::{
    module_factory::{CreateModuleContext, CreateModuleViewArgs},
    SharedOptions,
  },
  utils::parse_to_ecma_ast::{parse_to_ecma_ast, ParseToEcmaAstResult},
};

fn scan_ast(
  module_idx: ModuleIdx,
  id: &ArcStr,
  ast: &mut EcmaAst,
  symbol_table: SymbolTable,
  scopes: ScopeTree,
  module_def_format: ModuleDefFormat,
  options: &SharedOptions,
) -> BuildResult<(AstScopes, AstScanResult, SymbolRef)> {
  let module_id = ModuleId::new(id);
  let ast_scopes = AstScopes::new(scopes);

  let repr_name = module_id.as_path().representative_file_name();
  let repr_name = legitimize_identifier_name(&repr_name);

  let scanner = AstScanner::new(
    module_idx,
    &ast_scopes,
    symbol_table,
    &repr_name,
    module_def_format,
    ast.source(),
    &module_id,
    ast.comments(),
    options,
  );

  let namespace_object_ref = scanner.namespace_object_ref;
  let scan_result = scanner.scan(ast.program())?;

  Ok((ast_scopes, scan_result, namespace_object_ref))
}
pub struct CreateEcmaViewReturn {
  pub view: EcmaView,
  pub raw_import_records: IndexVec<ImportRecordIdx, RawImportRecord>,
  pub ast: EcmaAst,
  pub symbols: SymbolRefDbForModule,
  pub dynamic_import_exports_usage: FxHashMap<ImportRecordIdx, DynamicImportExportsUsage>,
}

pub async fn create_ecma_view(
  ctx: &mut CreateModuleContext<'_>,
  args: CreateModuleViewArgs,
) -> BuildResult<CreateEcmaViewReturn> {
  let id = ModuleId::new(&ctx.resolved_id.id);
  let stable_id = id.stabilize(&ctx.options.cwd);

  let parse_result = parse_to_ecma_ast(
    ctx.resolved_id.id.as_path(),
    &stable_id,
    ctx.options,
    &ctx.module_type,
    args.source.clone(),
    ctx.is_user_defined_entry,
  )?;

  let ParseToEcmaAstResult { mut ast, symbol_table, scope_tree, has_lazy_export, warning } =
    parse_result;

  ctx.warnings.extend(warning);

  let (scope, scan_result, namespace_object_ref) = scan_ast(
    ctx.module_index,
    &ctx.resolved_id.id,
    &mut ast,
    symbol_table,
    scope_tree,
    ctx.resolved_id.module_def_format,
    ctx.options,
  )?;

  let AstScanResult {
    named_imports,
    named_exports,
    stmt_infos,
    import_records,
    default_export_ref,
    imports,
    exports_kind,
    warnings: scan_warnings,
    has_eval,
    errors,
    ast_usage,
    symbols,
    self_referenced_class_decl_symbol_ids,
    hashbang_range,
    has_star_exports,
    dynamic_import_rec_exports_usage: dynamic_import_exports_usage,
    new_url_references: new_url_imports,
    this_expr_replace_map,
  } = scan_result;

  if !errors.is_empty() {
    return Err(errors.into());
  }

  ctx.warnings.extend(scan_warnings);

  // The side effects priority is:
  // 1. Hook side effects
  // 2. Package.json side effects
  // 3. Analyzed side effects
  // We should skip the `check_side_effects_for` if the hook side effects is not `None`.
  let lazy_check_side_effects = || {
    if matches!(ctx.module_type, ModuleType::Css) {
      // CSS modules are considered to have side effects by default
      return DeterminedSideEffects::Analyzed(true);
    }
    ctx
      .resolved_id
      .package_json
      .as_ref()
      .and_then(|p| {
        // the glob expr is based on parent path of package.json, which is package path
        // so we should use the relative path of the module to package path
        let module_path_relative_to_package = id.as_path().relative(p.path.parent()?);
        p.check_side_effects_for(&module_path_relative_to_package.to_string_lossy())
          .map(DeterminedSideEffects::UserDefined)
      })
      .unwrap_or_else(|| {
        let analyzed_side_effects = stmt_infos.iter().any(|stmt_info| stmt_info.side_effect);
        DeterminedSideEffects::Analyzed(analyzed_side_effects)
      })
  };

  let side_effects = match args.hook_side_effects {
    Some(side_effects) => match side_effects {
      HookSideEffects::True => lazy_check_side_effects(),
      HookSideEffects::False => DeterminedSideEffects::UserDefined(false),
      HookSideEffects::NoTreeshake => DeterminedSideEffects::NoTreeshake,
    },
    None => DeterminedSideEffects::NoTreeshake,
  };

  let view = EcmaView {
    source: ast.source().clone(),
    ecma_ast_idx: None,
    named_imports,
    named_exports,
    stmt_infos,
    imports,
    default_export_ref,
    scope,
    exports_kind,
    namespace_object_ref,
    def_format: ctx.resolved_id.module_def_format,
    import_records: IndexVec::default(),
    importers: FxIndexSet::default(),
    dynamic_importers: FxIndexSet::default(),
    imported_ids: FxIndexSet::default(),
    dynamically_imported_ids: FxIndexSet::default(),
    side_effects,
    ast_usage,
    self_referenced_class_decl_symbol_ids,
    hashbang_range,
    meta: {
      let mut meta = EcmaViewMeta::default();
      meta.set_included(false);
      meta.set_eval(has_eval);
      meta.set_has_lazy_export(has_lazy_export);
      meta.set_has_star_exports(has_star_exports);
      meta
    },
    mutations: vec![],
    new_url_references: new_url_imports,
    this_expr_replace_map,
  };

  Ok(CreateEcmaViewReturn {
    view,
    raw_import_records: import_records,
    ast,
    symbols,
    dynamic_import_exports_usage,
  })
}
