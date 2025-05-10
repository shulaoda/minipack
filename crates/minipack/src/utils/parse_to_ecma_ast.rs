use std::path::Path;

use arcstr::ArcStr;
use minipack_common::ModuleType;
use minipack_ecmascript::{EcmaAst, EcmaCompiler};
use minipack_error::BuildResult;
use oxc::{
  ast_visit::VisitMut as _,
  minifier::{CompressOptions, Compressor},
  semantic::{Scoping, SemanticBuilder},
  span::SourceType as OxcSourceType,
  transformer::{ESTarget, TransformOptions, Transformer},
};

use crate::scan_stage::ast_scanner::PreProcessor;

pub fn parse_to_ecma_ast(
  source: String,
  source_path: &Path,
  module_type: &ModuleType,
) -> BuildResult<(EcmaAst, Scoping)> {
  let source = if *module_type == ModuleType::Empty { ArcStr::new() } else { source.into() };
  let oxc_source_type = {
    let default = OxcSourceType::default().with_module(true);
    if let ModuleType::Ts = module_type { default.with_typescript(true) } else { default }
  };

  let mut ast = EcmaCompiler::parse(&source, oxc_source_type)?;

  let semantic_ret = ast.program.with_mut(|fields| SemanticBuilder::new().build(fields.program));
  if !semantic_ret.errors.is_empty() {
    Err(anyhow::anyhow!("Failed to parse, got: {:?}", semantic_ret.errors))?;
  }

  let stats = semantic_ret.semantic.stats();
  let mut scoping = semantic_ret.semantic.into_scoping();

  if matches!(module_type, ModuleType::Ts) {
    let transformer_return = ast.program.with_mut(|fields| {
      Transformer::new(fields.allocator, source_path, &TransformOptions::from(ESTarget::ESNext))
        .build_with_scoping(scoping, fields.program)
    });

    if !transformer_return.errors.is_empty() {
      Err(anyhow::anyhow!("Failed to transform, got: {:?}", transformer_return.errors))?;
    }

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

  Ok((ast, scoping))
}
