use std::path::Path;

use minipack_common::{ESTarget, NormalizedBundlerOptions};
use minipack_ecmascript::EcmaAst;
use minipack_error::BuildResult;
use oxc::ast_visit::VisitMut;
use oxc::diagnostics::Severity as OxcSeverity;
use oxc::minifier::{CompressOptions, Compressor};
use oxc::semantic::{SemanticBuilder, Stats};
use oxc::transformer::Transformer;

use super::ecmascript::EnsureSpanUniqueness;
use super::parse_to_ecma_ast::ParseToEcmaAstResult;

use crate::scan_stage::ast_scanner::pre_processor::PreProcessor;
use crate::types::oxc_parse_type::OxcParseType;

#[derive(Default)]
pub struct PreProcessEcmaAst {
  /// Only recreate semantic data if ast is changed.
  ast_changed: bool,

  /// Semantic statistics.
  stats: Stats,
}

impl PreProcessEcmaAst {
  pub fn build(
    &mut self,
    mut ast: EcmaAst,
    source_path: &Path,
    parsed_type: &OxcParseType,
    bundle_options: &NormalizedBundlerOptions,
  ) -> BuildResult<ParseToEcmaAstResult> {
    let mut warning = vec![];

    // Build initial semantic data and check for semantic errors.
    let semantic_ret = ast.program.with_mut(|fields| SemanticBuilder::new().build(fields.program));

    if !semantic_ret.errors.is_empty() {
      warning.extend(
        semantic_ret
          .errors
          .into_iter()
          .map(|error| anyhow::anyhow!("Parse failed, got: {:?}", error.message)),
      );
    }

    self.stats = semantic_ret.semantic.stats();
    let mut scoping = semantic_ret.semantic.into_scoping();

    // Transform TypeScript and jsx.
    if !matches!(parsed_type, OxcParseType::Js)
      || !matches!(bundle_options.target, ESTarget::EsNext)
    {
      let ret = ast.program.with_mut(|fields| {
        let mut transformer_options = bundle_options.base_transform_options.clone();
        // Auto enable jsx_plugin
        if matches!(parsed_type, OxcParseType::Tsx | OxcParseType::Jsx) {
          transformer_options.jsx.jsx_plugin = true;
        }
        Transformer::new(fields.allocator, source_path, &transformer_options)
          .build_with_scoping(scoping, fields.program)
      });

      let (errors, warnings) =
        ret.errors.into_iter().fold((Vec::new(), Vec::new()), |mut acc, item| {
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

      scoping = ret.scoping;
      self.ast_changed = true;
    }

    ast.program.with_mut(|fields| {
      // Perform dead code elimination.
      // NOTE: `CompressOptions::dead_code_elimination` will remove `ParenthesizedExpression`s from the AST.
      let compressor = Compressor::new(fields.allocator, CompressOptions::safest());
      if self.ast_changed {
        let semantic_ret = SemanticBuilder::new().with_stats(self.stats).build(fields.program);
        scoping = semantic_ret.semantic.into_scoping();
      }
      compressor.dead_code_elimination_with_scoping(scoping, fields.program);
    });

    ast.program.with_mut(|fields| {
      let mut pre_processor = PreProcessor::new(fields.allocator);
      pre_processor.visit_program(fields.program);
      ast.contains_use_strict = pre_processor.contains_use_strict;
    });

    ast.program.with_mut(|fields| {
      EnsureSpanUniqueness::new().visit_program(fields.program);
    });

    // NOTE: Recreate semantic data because AST is changed in the transformations above.
    scoping = ast.program.with_dependent(|_, dep| {
      SemanticBuilder::new()
        // Preallocate memory for the underlying data structures.
        .with_stats(self.stats)
        .build(&dep.program)
        .semantic
        .into_scoping()
    });

    Ok(ParseToEcmaAstResult { ast, scoping, warning })
  }
}
