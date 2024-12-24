use std::path::Path;

use itertools::Itertools;
use minipack_common::{ESTarget, NormalizedBundlerOptions};
use minipack_ecmascript::{EcmaAst, WithMutFields};
use minipack_error::BuildResult;
use oxc::ast::VisitMut;
use oxc::diagnostics::Severity as OxcSeverity;
use oxc::minifier::{CompressOptions, Compressor};
use oxc::semantic::{SemanticBuilder, Stats};
use oxc::transformer::{ESTarget as OxcESTarget, TransformOptions, Transformer};

use crate::ast_scanner::pre_processor::PreProcessor;
use crate::types::oxc_parse_type::OxcParseType;

use super::ecma_visitors::EnsureSpanUniqueness;
use super::parse_to_ecma_ast::ParseToEcmaAstResult;

#[derive(Default)]
pub struct PreProcessEcmaAst {
  /// Only recreate semantic data if ast is changed.
  ast_changed: bool,

  /// Semantic statistics.
  stats: Stats,
}

impl PreProcessEcmaAst {
  // #[allow(clippy::match_same_arms)]: `OxcParseType::Tsx` will have special logic to deal with ts compared to `OxcParseType::Jsx`
  #[allow(clippy::match_same_arms)]
  pub fn build(
    &mut self,
    mut ast: EcmaAst,
    parse_type: &OxcParseType,
    path: &str,
    bundle_options: &NormalizedBundlerOptions,
    has_lazy_export: bool,
  ) -> BuildResult<ParseToEcmaAstResult> {
    let mut warning = vec![];
    // Build initial semantic data and check for semantic errors.
    let semantic_ret =
      ast.program.with_mut(|WithMutFields { program, .. }| SemanticBuilder::new().build(program));
    if !semantic_ret.errors.is_empty() {
      warning.extend(
        semantic_ret
          .errors
          .iter()
          .map(|error| anyhow::anyhow!("Parse failed, got: {:?}", error.message)),
      );
    }

    self.stats = semantic_ret.semantic.stats();
    let (mut symbols, mut scopes) = semantic_ret.semantic.into_symbol_table_and_scope_tree();

    // Transform TypeScript and jsx.
    if !matches!(parse_type, OxcParseType::Js) || !matches!(bundle_options.target, ESTarget::EsNext)
    {
      let ret = ast.program.with_mut(|fields| {
        let target: OxcESTarget = bundle_options.target.into();
        let mut transformer_options = TransformOptions::from(target);
        // The oxc jsx_plugin is enabled by default, we need to disable it.
        transformer_options.jsx.jsx_plugin = false;

        Transformer::new(fields.allocator, Path::new(path), &transformer_options)
          .build_with_symbols_and_scopes(symbols, scopes, fields.program)
      });

      // TODO: emit diagnostic, aiming to pass more tests,
      // we ignore warning for now
      let errors = ret
        .errors
        .into_iter()
        .filter(|item| matches!(item.severity, OxcSeverity::Error))
        .collect_vec();

      if !errors.is_empty() {
        return Err(
          errors
            .iter()
            .map(|error| anyhow::anyhow!("Parse failed, got: {:?}", error.message))
            .collect::<Vec<anyhow::Error>>(),
        )?;
      }

      scopes = ret.scopes;
      symbols = ret.symbols;
      self.ast_changed = true;
    }

    ast.program.with_mut(|fields| -> BuildResult<()> {
      let WithMutFields { allocator, program, .. } = fields;

      if !has_lazy_export {
        // Perform dead code elimination.
        // NOTE: `CompressOptions::dead_code_elimination` will remove `ParenthesizedExpression`s from the AST.
        let compressor = Compressor::new(allocator, CompressOptions::all_false());
        if self.ast_changed {
          let semantic_ret = SemanticBuilder::new().with_stats(self.stats).build(program);
          (symbols, scopes) = semantic_ret.semantic.into_symbol_table_and_scope_tree();
        }
        compressor.dead_code_elimination_with_symbols_and_scopes(symbols, scopes, program);
      }

      Ok(())
    })?;

    ast.program.with_mut(|fields| {
      let mut pre_processor = PreProcessor::new(fields.allocator, false);
      pre_processor.visit_program(fields.program);
      ast.contains_use_strict = pre_processor.contains_use_strict;
    });

    ast.program.with_mut(|fields| {
      EnsureSpanUniqueness::new().visit_program(fields.program);
    });

    // NOTE: Recreate semantic data because AST is changed in the transformations above.
    let (symbol_table, scope_tree) = ast.program.with_dependent(|_owner, dep| {
      SemanticBuilder::new()
        // Required by `module.scope.get_child_ids` in `crates/rolldown/src/utils/renamer.rs`.
        .with_scope_tree_child_ids(true)
        // Preallocate memory for the underlying data structures.
        .with_stats(self.stats)
        .build(&dep.program)
        .semantic
        .into_symbol_table_and_scope_tree()
    });

    Ok(ParseToEcmaAstResult { ast, symbol_table, scope_tree, has_lazy_export, warning })
  }
}
