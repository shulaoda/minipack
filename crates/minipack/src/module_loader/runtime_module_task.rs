use tokio::sync::mpsc::Sender;

use minipack_common::ModuleLoaderMsg;
use minipack_common::{AstScopes, ModuleIdx, SymbolRef};
use minipack_ecmascript::EcmaAst;

use crate::types::SharedNormalizedBundlerOptions;

pub struct RuntimeModuleTask {
  module_id: ModuleIdx,
  tx: Sender<ModuleLoaderMsg>,
  options: SharedNormalizedBundlerOptions,
  // errors: Vec<BuildDiagnostic>,
}

pub struct MakeEcmaAstResult {
  ast: EcmaAst,
  ast_scope: AstScopes,
  // scan_result: ScanResult,
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
}
