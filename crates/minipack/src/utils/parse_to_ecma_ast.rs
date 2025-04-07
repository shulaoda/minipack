use arcstr::ArcStr;
use minipack_common::ModuleType;
use minipack_ecmascript::{EcmaAst, EcmaCompiler};
use minipack_error::BuildResult;
use oxc::{semantic::Scoping, span::SourceType as OxcSourceType};
use sugar_path::SugarPath;

use super::pre_process_ecma_ast::PreProcessEcmaAst;

use crate::types::{module_factory::CreateModuleContext, oxc_parse_type::OxcParseType};

pub struct ParseToEcmaAstResult {
  pub ast: EcmaAst,
  pub scoping: Scoping,
  pub has_lazy_export: bool,
  pub warning: Vec<anyhow::Error>,
}

pub fn parse_to_ecma_ast(
  ctx: &CreateModuleContext<'_>,
  source: String,
) -> BuildResult<ParseToEcmaAstResult> {
  let CreateModuleContext { options, stable_id, module_type, .. } = ctx;

  let (has_lazy_export, source, parsed_type) = pre_process_source(module_type, source)?;

  let oxc_source_type = {
    let default = OxcSourceType::default().with_module(true);
    match parsed_type {
      OxcParseType::Js | OxcParseType::Jsx => default,
      OxcParseType::Ts | OxcParseType::Tsx => default.with_typescript(true),
    }
  };

  let ecma_ast = match module_type {
    ModuleType::Json => EcmaCompiler::parse_expr_as_program(&source, oxc_source_type)?,
    _ => EcmaCompiler::parse(&source, oxc_source_type)?,
  };

  PreProcessEcmaAst::default().build(
    ecma_ast,
    stable_id.as_path(),
    &parsed_type,
    has_lazy_export,
    options,
  )
}

fn pre_process_source(
  module_type: &ModuleType,
  source: String,
) -> BuildResult<(bool, ArcStr, OxcParseType)> {
  let has_lazy_export = matches!(module_type, ModuleType::Json);

  let source = match module_type {
    ModuleType::Js | ModuleType::Jsx | ModuleType::Ts | ModuleType::Tsx | ModuleType::Json => {
      source
    }
    ModuleType::Empty => String::new(),
    ModuleType::Custom(custom_type) => {
      return Err(anyhow::format_err!("Unknown module type: {custom_type}"))?;
    }
  };

  Ok((has_lazy_export, source.into(), module_type.into()))
}
