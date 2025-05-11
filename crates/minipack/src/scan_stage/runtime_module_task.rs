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
      symbols,
      stmt_infos,
      imports,
      named_imports,
      named_exports,
      default_export_ref,
      namespace_object_ref,
      ..
    } = scan_result;

    let module = NormalModule {
      idx: self.idx,
      id: ModuleId::new(RUNTIME_MODULE_ID),
      stable_id: RUNTIME_MODULE_ID.to_string(),
      exec_order: u32::MAX,
      module_type: ModuleType::Js,
      is_user_defined_entry: false,
      ecma_view: EcmaView {
        source,
        imports,
        stmt_infos,
        named_imports,
        named_exports,
        default_export_ref,
        namespace_object_ref,
        ecma_ast_idx: None,
        meta: EcmaViewMeta::empty(),
        import_records: IndexVec::default(),
        side_effects: DeterminedSideEffects::Analyzed(false),
      },
    };

    let runtime = RuntimeModuleBrief::new(self.idx, &symbols.ast_scopes);
    let task_result = ModuleLoaderMsg::RuntimeModuleDone(Box::new(RuntimeModuleTaskResult {
      ast,
      module,
      runtime,
      symbols,
    }));

    let _ = self.tx.try_send(task_result);

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
