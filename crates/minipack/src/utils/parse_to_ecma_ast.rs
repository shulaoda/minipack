use std::path::Path;

use arcstr::ArcStr;
use minipack_common::ModuleType;
use minipack_ecmascript::{EcmaAst, EcmaCompiler};
use minipack_error::BuildResult;
use oxc::{
  ast_visit::VisitMut as _,
  diagnostics::Severity as OxcSeverity,
  minifier::{CompressOptions, Compressor},
  semantic::{Scoping, SemanticBuilder},
  span::SourceType as OxcSourceType,
  transformer::{ESTarget, TransformOptions, Transformer},
};

use crate::scan_stage::ast_scanner::pre_processor::PreProcessor;

pub struct ParseToEcmaAstResult {
  pub ast: EcmaAst,
  pub scoping: Scoping,
  pub warning: Vec<anyhow::Error>,
}

pub fn parse_to_ecma_ast(
  source_path: &Path,
  module_type: &ModuleType,
  source: String,
) -> BuildResult<ParseToEcmaAstResult> {
  let source = if *module_type == ModuleType::Empty { ArcStr::new() } else { source.into() };
  let oxc_source_type = {
    let default = OxcSourceType::default().with_module(true);
    if let ModuleType::Ts = module_type { default.with_typescript(true) } else { default }
  };

  let mut ast = EcmaCompiler::parse(&source, oxc_source_type)?;

  // Build initial semantic data and check for semantic errors.
  let semantic_ret = ast.program.with_mut(|fields| SemanticBuilder::new().build(fields.program));

  let mut warning = vec![];
  if !semantic_ret.errors.is_empty() {
    warning.extend(
      semantic_ret
        .errors
        .into_iter()
        .map(|error| anyhow::anyhow!("Parse failed, got: {:?}", error.message)),
    );
  }

  let stats = semantic_ret.semantic.stats();
  let mut scoping = semantic_ret.semantic.into_scoping();

  if matches!(module_type, ModuleType::Ts) {
    let transformer_return = ast.program.with_mut(|fields| {
      Transformer::new(fields.allocator, source_path, &TransformOptions::from(ESTarget::ESNext))
        .build_with_scoping(scoping, fields.program)
    });

    let (errors, warnings) =
      transformer_return.errors.into_iter().fold((Vec::new(), Vec::new()), |mut acc, item| {
        let message = anyhow::anyhow!("Parse failed, got: {:?}", item.message);
        if matches!(item.severity, OxcSeverity::Error) {
          acc.0.push(message);
        } else {
          acc.1.push(message);
        }
        acc
      });

    if !errors.is_empty() {
      Err(errors)?;
    }

    warning.extend(warnings);

    scoping = transformer_return.scoping;
    ast.program.with_mut(|fields| {
      let semantic_ret = SemanticBuilder::new().with_stats(stats).build(fields.program);
      scoping = semantic_ret.semantic.into_scoping();
    });
  }

  let scoping = ast.program.with_mut(|fields| {
    let compressor = Compressor::new(fields.allocator, CompressOptions::safest());
    compressor.dead_code_elimination_with_scoping(scoping, fields.program);

    PreProcessor::new(fields.allocator).visit_program(fields.program);
    SemanticBuilder::new().with_stats(stats).build(fields.program).semantic.into_scoping()
  });

  Ok(ParseToEcmaAstResult { ast, scoping, warning })
}
