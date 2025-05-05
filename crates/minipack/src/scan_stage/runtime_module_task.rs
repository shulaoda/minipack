use arcstr::ArcStr;
use minipack_common::side_effects::DeterminedSideEffects;
use minipack_error::BuildResult;
use minipack_utils::indexmap::FxIndexSet;
use oxc::ast_visit::VisitMut;
use oxc::semantic::SemanticBuilder;
use oxc::span::SourceType;
use oxc_index::IndexVec;
use rustc_hash::FxHashSet;
use tokio::sync::mpsc::Sender;

use minipack_common::{
  EcmaView, EcmaViewMeta, ModuleIdx, ModuleType, NormalModule, RUNTIME_MODULE_ID, ResolvedId,
  RuntimeModuleBrief, RuntimeModuleTaskResult,
};
use minipack_common::{ModuleId, ModuleLoaderMsg};
use minipack_ecmascript::{EcmaAst, EcmaCompiler};

use super::ast_scanner::{AstScanResult, AstScanner, pre_processor::PreProcessor};
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
    let source = arcstr::literal!(include_str!("./runtime/index.js"));

    let (ast, scan_result) = self.make_ecma_ast(&source)?;

    let AstScanResult {
      named_imports,
      named_exports,
      stmt_infos,
      default_export_ref,
      namespace_object_ref,
      has_top_level_await,
      symbols,
      imports,
      import_records: raw_import_records,
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
        namespace_object_ref,
        has_top_level_await,
        self_referenced_class_decl_symbol_ids: FxHashSet::default(),
        hashbang_range: None,
        meta: {
          let mut meta = EcmaViewMeta::default();
          meta.set(self::EcmaViewMeta::EVAL, has_eval);
          meta.set(self::EcmaViewMeta::HAS_STAR_EXPORT, has_star_exports);
          meta
        },
        this_expr_replace_map: FxHashSet::default(),
      },
    };

    let resolved_deps = raw_import_records
      .iter()
      .map(|rec| {
        // We assume the runtime module only has external dependencies.
        let id = rec.module_request.as_str().into();
        ResolvedId {
          id,
          ignored: false,
          is_external: true,
          package_json: None,
        }
      })
      .collect();

    let runtime = RuntimeModuleBrief::new(self.idx, &symbols.ast_scopes);
    let result = ModuleLoaderMsg::RuntimeModuleDone(RuntimeModuleTaskResult {
      ast,
      module,
      runtime,
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

    let scoping = ast.program.with_dependent(|_owner, dep| {
      SemanticBuilder::new().build(&dep.program).semantic.into_scoping()
    });

    let facade_path = ModuleId::new(RUNTIME_MODULE_ID);
    let scanner = AstScanner::new(
      self.idx,
      scoping,
      "minipack_runtime",
      source,
      &facade_path,
      ast.comments(),
      &self.options,
    );
    let scan_result = scanner.scan(ast.program())?;

    Ok((ast, scan_result))
  }
}
