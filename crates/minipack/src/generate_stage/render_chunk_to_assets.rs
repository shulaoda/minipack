use futures::future::try_join_all;
use minipack_common::{Asset, OutputChunk};
use minipack_ecmascript::EcmaCompiler;
use minipack_error::BuildResult;
use minipack_utils::rayon::{
  IntoParallelRefIterator, IntoParallelRefMutIterator, ParallelIterator,
};
use oxc_index::IndexVec;

use super::generators::ecmascript::EcmaGenerator;

use crate::{
  graph::ChunkGraph,
  types::{
    IndexInstantiatedChunks,
    bundle_output::BundleOutput,
    generator::{GenerateContext, Generator},
  },
  utils::chunk::finalize_chunks::finalize_assets,
};

use super::GenerateStage;

impl GenerateStage<'_> {
  pub async fn render_chunk_to_assets(
    &mut self,
    chunk_graph: &mut ChunkGraph,
  ) -> BuildResult<BundleOutput> {
    let mut warnings = std::mem::take(&mut self.link_output.warnings);

    let instantiated_chunks = self.instantiate_chunks(chunk_graph, &mut warnings).await?;

    let mut assets = finalize_assets(instantiated_chunks);

    if self.options.minify {
      assets.par_iter_mut().for_each(|asset| {
        asset.content = EcmaCompiler::minify(&asset.content, self.options.target.into());
      });
    }

    let mut output = Vec::with_capacity(assets.len());
    for Asset { content, filename } in assets {
      output.push(OutputChunk { filename: filename.into(), content });
    }

    Ok(BundleOutput { assets: output, warnings })
  }

  async fn instantiate_chunks(
    &self,
    chunk_graph: &ChunkGraph,
    warnings: &mut Vec<anyhow::Error>,
  ) -> BuildResult<IndexInstantiatedChunks> {
    let chunk_index_to_codegen_rets = self.create_chunk_to_codegen_ret_map(chunk_graph);
    let mut index_preliminary_assets = IndexVec::with_capacity(chunk_graph.chunk_table.len());

    let tasks = chunk_graph
      .chunk_table
      .iter_enumerated()
      .filter(|(_, chunk)| chunk.is_alive)
      .zip(chunk_index_to_codegen_rets.into_iter())
      .map(|((chunk_idx, chunk), module_id_to_codegen_ret)| async move {
        let mut ctx = GenerateContext {
          chunk_idx,
          chunk,
          options: self.options,
          link_output: self.link_output,
          chunk_graph,
          warnings: vec![],
          module_id_to_codegen_ret,
        };

        EcmaGenerator::instantiate_chunk(&mut ctx).await
      });

    for result in try_join_all(tasks).await? {
      index_preliminary_assets.extend(result.chunks);
      warnings.extend(result.warnings);
    }

    Ok(index_preliminary_assets)
  }

  /// Create a IndexVecMap from chunk index to related modules codegen return list.
  /// e.g.
  /// modules of chunk1: [ecma1, ecma2, external1]
  /// modules of chunk2: [ecma3, external2]
  /// ret: [
  ///   [Some(ecma1_codegen), Some(ecma2_codegen), None],
  ///   [Some(ecma3_codegen), None],
  /// ]
  fn create_chunk_to_codegen_ret_map(&self, chunk_graph: &ChunkGraph) -> Vec<Vec<Option<String>>> {
    chunk_graph
      .chunk_table
      .par_iter()
      .filter(|chunk| chunk.is_alive)
      .map(|item| {
        item
          .modules
          .par_iter()
          .map(|&module_idx| {
            if let Some(module) = self.link_output.module_table[module_idx].as_normal() {
              let ast = &self.link_output.index_ecma_ast[module.ecma_ast_idx()].0;
              Some(EcmaCompiler::print(ast).code)
            } else {
              None
            }
          })
          .collect::<Vec<_>>()
      })
      .collect::<Vec<_>>()
  }
}
