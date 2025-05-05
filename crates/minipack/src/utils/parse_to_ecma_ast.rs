use arcstr::ArcStr;
use minipack_common::ModuleType;
use minipack_ecmascript::{EcmaAst, EcmaCompiler};
use minipack_error::BuildResult;
use oxc::{semantic::Scoping, span::SourceType as OxcSourceType};
use sugar_path::SugarPath;

use super::pre_process_ecma_ast::PreProcessEcmaAst;

use crate::types::module_factory::CreateModuleContext;

pub struct ParseToEcmaAstResult {
  pub ast: EcmaAst,
  pub scoping: Scoping,
  pub warning: Vec<anyhow::Error>,
}

pub fn parse_to_ecma_ast(
  ctx: &CreateModuleContext<'_>,
  source: String,
) -> BuildResult<ParseToEcmaAstResult> {
  let CreateModuleContext { stable_id, module_type, .. } = ctx;

  let source = if matches!(module_type, ModuleType::Empty) { ArcStr::new() } else { source.into() };
  let oxc_source_type = {
    let default = OxcSourceType::default().with_module(true);
    if let ModuleType::Ts = module_type { default.with_typescript(true) } else { default }
  };

  let ecma_ast = EcmaCompiler::parse(&source, oxc_source_type)?;
  PreProcessEcmaAst::default().build(ecma_ast, stable_id.as_path(), module_type)
}
