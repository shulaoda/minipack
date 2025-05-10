use arcstr::ArcStr;
use minipack_common::side_effects::DeterminedSideEffects;
use minipack_error::BuildResult;
use oxc::ast_visit::VisitMut;
use oxc::semantic::SemanticBuilder;
use oxc::span::SourceType;
use oxc_index::IndexVec;
use tokio::sync::mpsc::Sender;

use minipack_common::{
  EcmaView, EcmaViewMeta, ModuleIdx, ModuleType, NormalModule, RUNTIME_MODULE_ID,
  RuntimeModuleBrief, RuntimeModuleTaskResult,
};
use minipack_common::{ModuleId, ModuleLoaderMsg};
use minipack_ecmascript::{EcmaAst, EcmaCompiler};

use super::ast_scanner::{AstScanResult, AstScanner, PreProcessor};

pub struct RuntimeModuleTask {
  idx: ModuleIdx,
  tx: Sender<ModuleLoaderMsg>,
}

impl RuntimeModuleTask {
  pub fn new(idx: ModuleIdx, tx: Sender<ModuleLoaderMsg>) -> Self {
    Self { idx, tx }
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
      symbols,
      imports,
      has_star_exports,
      ..
    } = scan_result;

    let module = NormalModule {
      idx: self.idx,
      stable_id: RUNTIME_MODULE_ID.to_string(),
      id: ModuleId::new(RUNTIME_MODULE_ID),
      exec_order: u32::MAX,
      is_user_defined_entry: false,
      module_type: ModuleType::Js,
      ecma_view: EcmaView {
        ecma_ast_idx: None,
        source,
        import_records: IndexVec::default(),
        side_effects: DeterminedSideEffects::Analyzed(false),
        imports,
        stmt_infos,
        named_imports,
        named_exports,
        default_export_ref,
        namespace_object_ref,
        meta: {
          let mut meta = EcmaViewMeta::default();
          meta.set(EcmaViewMeta::HAS_STAR_EXPORT, has_star_exports);
          meta
        },
      },
    };

    let runtime = RuntimeModuleBrief::new(self.idx, &symbols.ast_scopes);
    let result =
      ModuleLoaderMsg::RuntimeModuleDone(RuntimeModuleTaskResult { ast, module, runtime, symbols });

    let _ = self.tx.try_send(result);

    Ok(())
  }

  fn make_ecma_ast(&mut self, source: &ArcStr) -> BuildResult<(EcmaAst, AstScanResult)> {
    let source_type = SourceType::default();
    let mut ast = EcmaCompiler::parse(source, source_type)?;

    ast.program.with_mut(|fields| {
      let mut pre_processor = PreProcessor::new(fields.allocator);
      pre_processor.visit_program(fields.program);
    });

    let scoping = ast.program.with_dependent(|_owner, dep| {
      SemanticBuilder::new().build(&dep.program).semantic.into_scoping()
    });

    let ast_scanner = AstScanner::new(self.idx, scoping, "minipack_runtime");

    let ast_scan_result = ast_scanner.scan(ast.program())?;

    Ok((ast, ast_scan_result))
  }
}
