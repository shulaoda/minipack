use arcstr::ArcStr;
use minipack_common::side_effects::DeterminedSideEffects;
use minipack_error::BuildResult;
use minipack_utils::indexmap::FxIndexSet;
use oxc::ast::VisitMut;
use oxc::span::SourceType;
use oxc_index::IndexVec;
use rustc_hash::{FxHashMap, FxHashSet};
use tokio::sync::mpsc::Sender;

use minipack_common::{
  EcmaView, EcmaViewMeta, ExportsKind, ModuleIdx, ModuleType, NormalModule, ResolvedId,
  RuntimeModuleBrief, RuntimeModuleTaskResult, RUNTIME_MODULE_ID,
};
use minipack_common::{ModuleDefFormat, ModuleId, ModuleLoaderMsg};
use minipack_ecmascript::{EcmaAst, EcmaCompiler};

use crate::module_loader::ast_scanner::pre_processor::PreProcessor;
use crate::module_loader::ast_scanner::{AstScanResult, AstScanner};
use crate::types::SharedNormalizedBundlerOptions;

pub struct RuntimeModuleTask {
  idx: ModuleIdx,
  tx: Sender<ModuleLoaderMsg>,
  options: SharedNormalizedBundlerOptions,
}

impl RuntimeModuleTask {
  pub fn new(
    idx: ModuleIdx,
    tx: Sender<ModuleLoaderMsg>,
    options: SharedNormalizedBundlerOptions,
  ) -> Self {
    Self { idx, tx, options }
  }

  pub fn run(mut self) {
    if let Err(errs) = self.run_inner() {
      self.tx.try_send(ModuleLoaderMsg::BuildErrors(errs.0)).expect("Send should not fail");
    }
  }

  fn run_inner(&mut self) -> BuildResult<()> {
    let source = if self.options.is_esm_format_with_node_platform() {
      arcstr::literal!(concat!(
        include_str!("../runtime/runtime-head-node.js"),
        include_str!("../runtime/runtime-base.js"),
        include_str!("../runtime/runtime-tail-node.js"),
      ))
    } else {
      arcstr::literal!(concat!(
        include_str!("../runtime/runtime-base.js"),
        include_str!("../runtime/runtime-tail.js"),
      ))
    };

    let (ast, scan_result) = self.make_ecma_ast(&source)?;

    let AstScanResult {
      named_imports,
      named_exports,
      stmt_infos,
      default_export_ref,
      namespace_object_ref,
      ast_usage,
      scopes,
      symbols,
      imports,
      import_records: raw_import_records,
      new_url_references,
      has_star_exports,
      has_eval,
      ..
    } = scan_result;

    let module = NormalModule {
      idx: self.idx,
      repr_name: "rolldown_runtime".to_string(),
      stable_id: RUNTIME_MODULE_ID.to_string(),
      id: ModuleId::new(RUNTIME_MODULE_ID),
      debug_id: RUNTIME_MODULE_ID.to_string(),
      exec_order: u32::MAX,
      is_user_defined_entry: false,
      module_type: ModuleType::Js,
      ecma_view: EcmaView {
        ecma_ast_idx: None,
        source,
        import_records: IndexVec::default(),
        // The internal runtime module `importers/imported` should be skip.
        importers: FxIndexSet::default(),
        dynamic_importers: FxIndexSet::default(),
        imported_ids: FxIndexSet::default(),
        dynamically_imported_ids: FxIndexSet::default(),
        side_effects: DeterminedSideEffects::Analyzed(false),
        named_imports,
        named_exports,
        stmt_infos,
        imports,
        default_export_ref,
        ast_scope_idx: None,
        exports_kind: ExportsKind::Esm,
        namespace_object_ref,
        def_format: ModuleDefFormat::EsmMjs,
        ast_usage,
        self_referenced_class_decl_symbol_ids: FxHashSet::default(),
        hashbang_range: None,
        meta: {
          let mut meta = EcmaViewMeta::default();
          meta.set_included(false);
          meta.set_eval(has_eval);
          meta.set_has_lazy_export(false);
          meta.set_has_star_exports(has_star_exports);
          meta
        },
        mutations: vec![],
        new_url_references,
        this_expr_replace_map: FxHashMap::default(),
        esm_namespace_in_cjs: None,
        esm_namespace_in_cjs_node_mode: None,
      },
      css_view: None,
      asset_view: None,
    };

    let resolved_deps = raw_import_records
      .iter()
      .map(|rec| {
        // We assume the runtime module only has external dependencies.
        let id = rec.module_request.to_string().into();
        ResolvedId {
          id,
          ignored: false,
          module_def_format: ModuleDefFormat::Unknown,
          is_external: true,
          package_json: None,
          is_external_without_side_effects: true,
        }
      })
      .collect();

    let runtime = RuntimeModuleBrief::new(self.idx, &scopes);
    let result = ModuleLoaderMsg::RuntimeModuleDone(RuntimeModuleTaskResult {
      ast,
      module,
      runtime,
      scopes,
      symbols,
      resolved_deps,
      raw_import_records,
    });

    let _ = self.tx.try_send(result);

    Ok(())
  }

  fn make_ecma_ast(&mut self, source: &ArcStr) -> BuildResult<(EcmaAst, AstScanResult)> {
    let source_type = SourceType::default();

    let mut ast = EcmaCompiler::parse(source, source_type)?;

    ast.program.with_mut(|fields| {
      let mut pre_processor = PreProcessor::new(fields.allocator);
      pre_processor.visit_program(fields.program);
      ast.contains_use_strict = pre_processor.contains_use_strict;
    });

    let (symbol_table, scopes) = ast.make_symbol_table_and_scope_tree();
    let facade_path = ModuleId::new("runtime");
    let scanner = AstScanner::new(
      self.idx,
      scopes,
      symbol_table,
      "rolldown_runtime",
      ModuleDefFormat::EsmMjs,
      source,
      &facade_path,
      ast.comments(),
      &self.options,
    );
    let scan_result = scanner.scan(ast.program())?;

    Ok((ast, scan_result))
  }
}
