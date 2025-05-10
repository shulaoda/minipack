use minipack_ecmascript::EcmaAst;

use crate::{NormalModule, SymbolRefDbForModule};

use super::runtime_module_brief::RuntimeModuleBrief;

pub struct RuntimeModuleTaskResult {
  pub ast: EcmaAst,
  pub module: NormalModule,
  pub runtime: RuntimeModuleBrief,
  pub symbols: SymbolRefDbForModule,
}
