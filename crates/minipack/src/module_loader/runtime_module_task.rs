use arcstr::ArcStr;
use oxc::span::SourceType;
use tokio::sync::mpsc::Sender;

use minipack_common::ModuleLoaderMsg;
use minipack_common::{AstScopes, ModuleIdx, SymbolRef};
use minipack_ecmascript::{EcmaAst, EcmaCompiler};

use crate::ast_scanner::pre_processor::PreProcessor;
use crate::types::{BuildResult, SharedNormalizedBundlerOptions};

pub struct RuntimeModuleTask {
  module_id: ModuleIdx,
  tx: Sender<ModuleLoaderMsg>,
  options: SharedNormalizedBundlerOptions,
  // errors: Vec<BuildDiagnostic>,
}

pub struct MakeEcmaAstResult {
  ast: EcmaAst,
  ast_scope: AstScopes,
  scan_result: ScanResult,
  namespace_object_ref: SymbolRef,
}

impl RuntimeModuleTask {
  pub fn new(
    id: ModuleIdx,
    tx: tokio::sync::mpsc::Sender<ModuleLoaderMsg>,
    options: SharedNormalizedBundlerOptions,
  ) -> Self {
    Self { module_id: id, tx, options }
  }

  pub fn run(mut self) -> anyhow::Result<()> {
    todo!()
  }

  fn make_ecma_ast(&mut self, source: &ArcStr) -> BuildResult<MakeEcmaAstResult> {
    let source_type = SourceType::default();

    let mut ast = EcmaCompiler::parse(source, source_type)?;

    ast.program.with_mut(|fields| {
      let mut pre_processor = PreProcessor::new(fields.allocator, false);
      pre_processor.visit_program(fields.program);
      ast.contains_use_strict = pre_processor.contains_use_strict;
    });

    let (mut symbol_table, scope) = ast.make_symbol_table_and_scope_tree();
    let ast_scope = AstScopes::new(
      scope,
      std::mem::take(&mut symbol_table.references),
      std::mem::take(&mut symbol_table.resolved_references),
    );
    let facade_path = ModuleId::new("runtime");
    let scanner = AstScanner::new(
      self.module_id,
      &ast_scope,
      symbol_table,
      "rolldown_runtime",
      ModuleDefFormat::EsmMjs,
      source,
      &facade_path,
      ast.comments(),
      &self.options,
    );
    let namespace_object_ref = scanner.namespace_object_ref;
    let scan_result = scanner.scan(ast.program())?;

    Ok(MakeEcmaAstResult { ast, ast_scope, scan_result, namespace_object_ref })
  }
}
