mod code_splitting;
mod compute_cross_chunk_links;
mod generate_chunk_name_and_preliminary_filenames;
mod render_chunk_to_assets;
mod scope_hoisting;

pub mod generators;

use minipack_common::Module;
use minipack_ecmascript::AstSnippet;
use minipack_error::BuildResult;
use minipack_utils::rayon::{IntoParallelRefMutIterator, ParallelIterator};
use oxc::ast_visit::VisitMut;
use rustc_hash::FxHashSet;

use crate::{
  types::{SharedOptions, bundle_output::BundleOutput},
  utils::chunk::deconflict_chunk_symbols::deconflict_chunk_symbols,
};

use super::link_stage::LinkStageOutput;

pub struct GenerateStage {
  options: SharedOptions,
  link_stage_output: LinkStageOutput,
}

impl GenerateStage {
  pub fn new(link_stage_output: LinkStageOutput, options: SharedOptions) -> Self {
    Self { link_stage_output, options }
  }

  pub async fn generate(&mut self) -> BuildResult<BundleOutput> {
    if !self.link_stage_output.errors.is_empty() {
      return Err(std::mem::take(&mut self.link_stage_output.errors))?;
    }

    let mut chunk_graph = self.generate_chunks().await;

    self.compute_cross_chunk_links(&mut chunk_graph);

    let chunk_id_to_name =
      self.generate_chunk_name_and_preliminary_filenames(&mut chunk_graph).await?;

    chunk_graph.chunk_table.par_iter_mut().for_each(|chunk| {
      deconflict_chunk_symbols(
        chunk,
        &self.link_stage_output,
        self.options.format,
        &chunk_id_to_name,
      );
    });

    self.link_stage_output.ecma_ast.par_iter_mut().for_each(|(ast, owner)| {
      let Module::Normal(module) = &self.link_stage_output.module_table[*owner] else {
        return;
      };

      if !module.meta.is_included() {
        return;
      }

      let chunk_id = chunk_graph.module_to_chunk[module.idx].unwrap();
      let linking_info = &self.link_stage_output.metadata[module.idx];
      let canonical_names = &chunk_graph.chunk_table[chunk_id].canonical_names;
      ast.program.with_mut(|fields| {
        let (oxc_program, allocator) = (fields.program, fields.allocator);
        let mut finalizer = scope_hoisting::ScopeHoistingFinalizer {
          allocator,
          snippet: AstSnippet::new(allocator),
          ast_scope: &self.link_stage_output.symbol_ref_db[module.idx].as_ref().unwrap().ast_scopes,
          ctx: scope_hoisting::ScopeHoistingFinalizerContext {
            canonical_names,
            id: module.idx,
            chunk_id,
            symbol_ref_db: &self.link_stage_output.symbol_ref_db,
            linking_info,
            module,
            modules: &self.link_stage_output.module_table,
            runtime: &self.link_stage_output.runtime_module,
            chunk_graph: &chunk_graph,
            options: &self.options,
          },
          namespace_alias_symbol_id: FxHashSet::default(),
        };
        finalizer.visit_program(oxc_program);
      });
    });

    self.render_chunk_to_assets(&mut chunk_graph).await
  }
}
