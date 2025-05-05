mod code_splitting;
mod compute_cross_chunk_links;
mod generate_chunk_name_and_preliminary_filenames;
mod render_chunk_to_assets;
mod scope_hoisting;

pub mod generators;

use scope_hoisting::{ScopeHoistingFinalizer, ScopeHoistingFinalizerContext};

use oxc::{allocator::TakeIn, ast_visit::VisitMut};
use rustc_hash::FxHashSet;

use minipack_common::Module;
use minipack_ecmascript::AstSnippet;
use minipack_error::BuildResult;
use minipack_utils::rayon::{IntoParallelRefMutIterator, ParallelIterator};

use crate::{
  types::{SharedOptions, bundle_output::BundleOutput},
  utils::chunk::{
    deconflict_chunk_symbols::deconflict_chunk_symbols,
    validate_options_for_multi_chunk_output::validate_options_for_multi_chunk_output,
  },
};

use super::link_stage::LinkStageOutput;

pub struct GenerateStage<'a> {
  link_output: &'a mut LinkStageOutput,
  options: &'a SharedOptions,
}

impl<'a> GenerateStage<'a> {
  pub fn new(link_output: &'a mut LinkStageOutput, options: &'a SharedOptions) -> Self {
    Self { link_output, options }
  }

  pub async fn generate(&mut self) -> BuildResult<BundleOutput> {
    if !self.link_output.errors.is_empty() {
      return Err(std::mem::take(&mut self.link_output.errors))?;
    }

    let mut chunk_graph = self.generate_chunks().await;
    if chunk_graph.chunk_table.len() > 1 {
      validate_options_for_multi_chunk_output(self.options)?;
    }

    self.compute_cross_chunk_links(&mut chunk_graph);

    let index_chunk_id_to_name =
      self.generate_chunk_name_and_preliminary_filenames(&mut chunk_graph).await?;

    chunk_graph.chunk_table.par_iter_mut().for_each(|chunk| {
      deconflict_chunk_symbols(
        chunk,
        self.link_output,
        self.options.format,
        &index_chunk_id_to_name,
      );
    });

    let ast_table_iter = self.link_output.index_ecma_ast.par_iter_mut();
    ast_table_iter.for_each(|(ast, owner)| {
      let Module::Normal(module) = &self.link_output.module_table[*owner] else {
        return;
      };

      if !module.meta.is_included() {
        return;
      }

      let ast_scope = &self.link_output.symbols[module.idx].as_ref().unwrap().ast_scopes;
      let chunk_id = chunk_graph.module_to_chunk[module.idx].unwrap();
      let chunk = &chunk_graph.chunk_table[chunk_id];
      let linking_info = &self.link_output.metadata[module.idx];
      ast.program.with_mut(|fields| {
        let (oxc_program, alloc) = (fields.program, fields.allocator);
        let mut finalizer = ScopeHoistingFinalizer {
          alloc,
          ctx: ScopeHoistingFinalizerContext {
            canonical_names: &chunk.canonical_names,
            id: module.idx,
            chunk_id,
            symbol_db: &self.link_output.symbols,
            linking_info,
            module,
            modules: &self.link_output.module_table,
            linking_infos: &self.link_output.metadata,
            runtime: &self.link_output.runtime_module,
            chunk_graph: &chunk_graph,
            options: self.options,
          },
          scope: ast_scope,
          snippet: AstSnippet::new(alloc),
          comments: oxc_program.comments.take_in(alloc),
          namespace_alias_symbol_id: FxHashSet::default(),
          interested_namespace_alias_ref_id: FxHashSet::default(),
        };
        finalizer.visit_program(oxc_program);
        oxc_program.comments = finalizer.comments.take_in(alloc);
      });
    });

    self.render_chunk_to_assets(&mut chunk_graph).await
  }
}
