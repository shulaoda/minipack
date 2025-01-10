use futures::future::try_join_all;
use minipack_common::{
  Asset, InstantiationKind, ModuleRenderArgs, Output, OutputAsset, OutputChunk,
};
use minipack_ecmascript::EcmaCompiler;
use minipack_error::BuildResult;
use minipack_utils::{
  indexmap::FxIndexSet,
  rayon::{IntoParallelRefIterator, IntoParallelRefMutIterator, ParallelIterator},
};
use oxc_index::{index_vec, IndexVec};

use crate::{
  graph::ChunkGraph,
  module_loader::loaders::{
    asset::asset_generator::AssetGenerator, css::css_generator::CssGenerator,
    ecmascript::ecma_generator::EcmaGenerator,
  },
  types::{
    bundle_output::BundleOutput,
    generator::{GenerateContext, Generator},
    IndexAssets, IndexChunkToAssets, IndexInstantiatedChunks,
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
    let (instantiated_chunks, index_chunk_to_assets) =
      self.instantiate_chunks(chunk_graph, &mut warnings).await?;

    let mut assets = finalize_assets(chunk_graph, instantiated_chunks, &index_chunk_to_assets);

    self.minify_assets(&mut assets)?;

    let mut output = Vec::with_capacity(assets.len());
    for Asset { meta: rendered_chunk, content: code, preliminary_filename, filename, .. } in assets
    {
      if let InstantiationKind::Ecma(ecma_meta) = rendered_chunk {
        let code = code.try_into_string()?;
        let rendered_chunk = ecma_meta.rendered_chunk;

        output.push(Output::Chunk(Box::new(OutputChunk {
          name: rendered_chunk.name,
          filename: rendered_chunk.filename,
          code,
          is_entry: rendered_chunk.is_entry,
          is_dynamic_entry: rendered_chunk.is_dynamic_entry,
          facade_module_id: rendered_chunk.facade_module_id,
          modules: rendered_chunk.modules,
          exports: rendered_chunk.exports,
          module_ids: rendered_chunk.module_ids,
          imports: rendered_chunk.imports,
          dynamic_imports: rendered_chunk.dynamic_imports,
          preliminary_filename: preliminary_filename.to_string(),
        })));
      } else {
        output.push(Output::Asset(Box::new(OutputAsset {
          filename: filename.clone().into(),
          source: code,
          original_file_names: vec![],
          names: vec![],
        })));
      }
    }

    // The chunks order make sure the entry chunk at first, the assets at last, see https://github.com/rollup/rollup/blob/master/src/rollup/rollup.ts#L266
    output.sort_unstable_by(|a, b| {
      let a_type = get_sorting_file_type(a) as u8;
      let b_type = get_sorting_file_type(b) as u8;
      if a_type == b_type {
        return a.filename().cmp(b.filename());
      }
      a_type.cmp(&b_type)
    });

    Ok(BundleOutput { assets: output, warnings })
  }

  async fn instantiate_chunks(
    &self,
    chunk_graph: &ChunkGraph,
    warnings: &mut Vec<anyhow::Error>,
  ) -> BuildResult<(IndexInstantiatedChunks, IndexChunkToAssets)> {
    let mut index_chunk_to_assets: IndexChunkToAssets =
      index_vec![FxIndexSet::default(); chunk_graph.chunk_table.len()];
    let mut index_preliminary_assets: IndexInstantiatedChunks =
      IndexVec::with_capacity(chunk_graph.chunk_table.len());
    let chunk_index_to_codegen_rets = self.create_chunk_to_codegen_ret_map(chunk_graph);

    try_join_all(
      chunk_graph.chunk_table.iter_enumerated().zip(chunk_index_to_codegen_rets.into_iter()).map(
        |((chunk_idx, chunk), module_id_to_codegen_ret)| async move {
          let mut ctx = GenerateContext {
            chunk_idx,
            chunk,
            options: self.options,
            link_output: self.link_output,
            chunk_graph,
            warnings: vec![],
            module_id_to_codegen_ret,
          };
          let ecma_chunks = EcmaGenerator::instantiate_chunk(&mut ctx).await;

          let mut ctx = GenerateContext {
            chunk_idx,
            chunk,
            options: self.options,
            link_output: self.link_output,
            chunk_graph,
            warnings: vec![],
            // FIXME: module_id_to_codegen_ret is currently not used in CssGenerator. But we need to pass it to satisfy the args.
            module_id_to_codegen_ret: vec![],
          };
          let css_chunks = CssGenerator::instantiate_chunk(&mut ctx).await;

          let mut ctx = GenerateContext {
            chunk_idx,
            chunk,
            options: self.options,
            link_output: self.link_output,
            chunk_graph,
            warnings: vec![],
            // FIXME: module_id_to_codegen_ret is currently not used in AssetGenerator. But we need to pass it to satisfy the args.
            module_id_to_codegen_ret: vec![],
          };
          let asset_chunks = AssetGenerator::instantiate_chunk(&mut ctx).await;

          ecma_chunks.and_then(|ecma_chunks| {
            css_chunks.and_then(|css_chunks| {
              asset_chunks.map(|asset_chunks| [ecma_chunks, css_chunks, asset_chunks])
            })
          })
        },
      ),
    )
    .await?
    .into_iter()
    .flatten()
    .for_each(|result| {
      result.chunks.into_iter().for_each(|asset| {
        let origin_chunk = asset.origin_chunk;
        let asset_idx = index_preliminary_assets.push(asset);
        index_chunk_to_assets[origin_chunk].insert(asset_idx);
      });
      warnings.extend(result.warnings);
    });

    index_chunk_to_assets.iter_mut().for_each(|assets| {
      assets.sort_by_cached_key(|asset_idx| {
        index_preliminary_assets[*asset_idx].preliminary_filename.as_str()
      });
    });

    Ok((index_preliminary_assets, index_chunk_to_assets))
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
    let chunk_to_codegen_ret = chunk_graph
      .chunk_table
      .par_iter()
      .map(|item| {
        item
          .modules
          .par_iter()
          .map(|&module_idx| {
            if let Some(module) = self.link_output.module_table[module_idx].as_normal() {
              let ast = &self.link_output.index_ecma_ast[module.ecma_ast_idx()].0;
              module.render(&ModuleRenderArgs::Ecma { ast })
            } else {
              None
            }
          })
          .collect::<Vec<_>>()
      })
      .collect::<Vec<_>>();
    chunk_to_codegen_ret
  }

  pub fn minify_assets(&mut self, assets: &mut IndexAssets) -> BuildResult<()> {
    if self.options.minify {
      assets.par_iter_mut().try_for_each(|asset| -> anyhow::Result<()> {
        match asset.meta {
          InstantiationKind::Ecma(_) => {
            let minified_content = EcmaCompiler::minify(asset.content.try_as_inner_str()?);
            asset.content = minified_content.into();
          }
          InstantiationKind::None => {}
        }
        Ok(())
      })?;
    }

    Ok(())
  }
}

enum SortingFileType {
  EntryChunk = 0,
  SecondaryChunk = 1,
  Asset = 2,
}

#[inline]
fn get_sorting_file_type(output: &Output) -> SortingFileType {
  match output {
    Output::Asset(_) => SortingFileType::Asset,
    Output::Chunk(chunk) => {
      if chunk.is_entry {
        SortingFileType::EntryChunk
      } else {
        SortingFileType::SecondaryChunk
      }
    }
  }
}
