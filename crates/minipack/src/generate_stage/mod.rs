mod code_splitting;
mod compute_cross_chunk_links;
mod finalizers;
mod generate_chunk_name_and_preliminary_filenames;
mod render_chunk_to_assets;

pub mod generators;

use finalizers::{
  isolating::{IsolatingModuleFinalizer, IsolatingModuleFinalizerContext},
  scope_hoisting::{ScopeHoistingFinalizer, ScopeHoistingFinalizerContext},
};

use arcstr::ArcStr;
use minipack_common::{CssAssetNameReplacer, ImportMetaRolldownAssetReplacer, Module};
use minipack_ecmascript_utils::{AstSnippet, TakeIn};
use minipack_error::BuildResult;
use minipack_utils::rayon::{IntoParallelRefMutIterator, ParallelIterator};
use oxc::ast_visit::VisitMut;
use rustc_hash::{FxHashMap, FxHashSet};

use crate::{
  graph::ChunkGraph,
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
    self.patch_asset_modules(&chunk_graph);

    chunk_graph.chunk_table.par_iter_mut().for_each(|chunk| {
      deconflict_chunk_symbols(
        chunk,
        self.link_output,
        self.options.format,
        &index_chunk_id_to_name,
      );
    });

    let ast_table_iter = self.link_output.index_ecma_ast.par_iter_mut();
    ast_table_iter
      .filter(|(_ast, owner)| {
        self.link_output.modules[*owner].as_normal().is_some_and(|m| m.meta.is_included())
      })
      .for_each(|(ast, owner)| {
        let Module::Normal(module) = &self.link_output.modules[*owner] else {
          return;
        };
        let ast_scope_idx = module.ecma_view.ast_scope_idx.expect("scope idx should be set");
        let ast_scope = &self.link_output.ast_scope_table[ast_scope_idx];
        let chunk_id = chunk_graph.module_to_chunk[module.idx].unwrap();
        let chunk = &chunk_graph.chunk_table[chunk_id];
        let linking_info = &self.link_output.metadata[module.idx];
        if self.options.format.requires_scope_hoisting() {
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
                modules: &self.link_output.modules,
                linking_infos: &self.link_output.metadata,
                runtime: &self.link_output.runtime_module,
                chunk_graph: &chunk_graph,
                options: self.options,
                cur_stmt_index: 0,
                keep_name_statement_to_insert: Vec::new(),
              },
              scope: ast_scope,
              snippet: AstSnippet::new(alloc),
              comments: oxc_program.comments.take_in(alloc),
              namespace_alias_symbol_id: FxHashSet::default(),
              interested_namespace_alias_ref_id: FxHashSet::default(),
              generated_init_esm_importee_ids: FxHashSet::default(),
            };
            finalizer.visit_program(oxc_program);
            oxc_program.comments = finalizer.comments.take_in(alloc);
          });
        } else {
          ast.program.with_mut(|fields| {
            let (oxc_program, alloc) = (fields.program, fields.allocator);
            let mut finalizer = IsolatingModuleFinalizer {
              alloc,
              scope: ast_scope,
              ctx: &IsolatingModuleFinalizerContext {
                module,
                modules: &self.link_output.modules,
                symbol_db: &self.link_output.symbols,
              },
              snippet: AstSnippet::new(alloc),
              generated_imports_set: FxHashSet::default(),
              generated_imports: oxc::allocator::Vec::new_in(alloc),
              generated_exports: oxc::allocator::Vec::new_in(alloc),
            };
            finalizer.visit_program(oxc_program);
          });
        }
      });

    self.render_chunk_to_assets(&mut chunk_graph).await
  }

  pub fn patch_asset_modules(&mut self, chunk_graph: &ChunkGraph) {
    chunk_graph.chunk_table.iter().for_each(|chunk| {
      let mut module_idx_to_filenames = FxHashMap::default();
      // replace asset name in ecma view
      chunk.asset_preliminary_filenames.iter().for_each(|(module_idx, preliminary)| {
        let Module::Normal(module) = &mut self.link_output.modules[*module_idx] else {
          return;
        };
        let asset_filename: ArcStr = preliminary.as_str().into();
        module.ecma_view.mutations.push(Box::new(ImportMetaRolldownAssetReplacer {
          asset_filename: asset_filename.clone(),
        }));
        module_idx_to_filenames.insert(module_idx, asset_filename);
      });
      // replace asset name in css view
      chunk.modules.iter().for_each(|module_idx| {
        let module = &mut self.link_output.modules[*module_idx];
        if let Some(css_view) =
          module.as_normal_mut().and_then(|normal_module| normal_module.css_view.as_mut())
        {
          for (idx, record) in css_view.import_records.iter_enumerated() {
            if let Some(asset_filename) = module_idx_to_filenames.get(&record.resolved_module) {
              let span = css_view.record_idx_to_span[idx];
              css_view
                .mutations
                .push(Box::new(CssAssetNameReplacer { span, asset_name: asset_filename.clone() }));
            }
          }
        }
      });
    });
  }
}
