use std::sync::Arc;

use crate::{
  types::generator::{GenerateContext, GenerateOutput, Generator},
  utils::{
    chunk::generate_rendered_chunk::generate_rendered_chunk, render_ecma_module::render_ecma_module,
  },
};

use anyhow::Result;

use minipack_common::{
  EcmaAssetMeta, InstantiatedChunk, InstantiationKind, ModuleId, ModuleIdx, OutputFormat,
  RenderedModule,
};
use minipack_error::BuildResult;
use minipack_sourcemap::Source;
use minipack_utils::rayon::{IndexedParallelIterator, IntoParallelRefIterator, ParallelIterator};
use rustc_hash::FxHashMap;

use super::format::{cjs::render_cjs, esm::render_esm};

pub type RenderedModuleSources =
  Vec<(ModuleIdx, ModuleId, Option<Arc<[Box<dyn Source + Send + Sync>]>>)>;

pub struct EcmaGenerator;

impl Generator for EcmaGenerator {
  #[allow(clippy::too_many_lines)]
  async fn instantiate_chunk<'a>(
    ctx: &mut GenerateContext<'a>,
  ) -> Result<BuildResult<GenerateOutput>> {
    let mut rendered_modules = FxHashMap::default();
    let module_id_to_codegen_ret = std::mem::take(&mut ctx.module_id_to_codegen_ret);
    let rendered_module_sources = ctx
      .chunk
      .modules
      .par_iter()
      .copied()
      .zip(module_id_to_codegen_ret)
      .filter_map(|(id, codegen_ret)| {
        ctx.link_output.module_table.modules[id]
          .as_normal()
          .map(|m| (m, codegen_ret.expect("should have codegen_ret")))
      })
      .map(|(m, codegen_ret)| (m.idx, m.id.clone(), render_ecma_module(m, codegen_ret)))
      .collect::<Vec<_>>();

    rendered_module_sources.iter().for_each(|(_, module_id, sources)| {
      rendered_modules.insert(module_id.clone(), RenderedModule::new(sources.clone()));
    });

    let rendered_chunk = generate_rendered_chunk(
      ctx.chunk,
      rendered_modules,
      ctx.chunk.pre_rendered_chunk.as_ref().expect("Should have pre-rendered chunk"),
      ctx.graph,
    );

    let mut warnings = vec![];

    let source_joiner = match ctx.options.format {
      OutputFormat::Esm => render_esm(ctx, &rendered_module_sources),
      OutputFormat::Cjs => match render_cjs(ctx, &rendered_module_sources, &mut warnings) {
        Ok(source_joiner) => source_joiner,
        Err(errors) => return Ok(Err(errors)),
      },
    };

    ctx.warnings.extend(warnings);

    let content = source_joiner.join();

    // Here file path is generated by chunk file name template, it maybe including path segments.
    // So here need to read it's parent directory as file_dir.
    let file_path = ctx.options.cwd.as_path().join(&ctx.options.dir).join(
      ctx
        .chunk
        .preliminary_filename
        .as_deref()
        .expect("chunk file name should be generated before rendering")
        .as_str(),
    );
    let file_dir = file_path.parent().expect("chunk file name should have a parent");

    Ok(Ok(GenerateOutput {
      chunks: vec![InstantiatedChunk {
        origin_chunk: ctx.chunk_idx,
        content: content.into(),
        kind: InstantiationKind::from(EcmaAssetMeta { rendered_chunk }),
        augment_chunk_hash: None,
        file_dir: file_dir.to_path_buf(),
        preliminary_filename: ctx
          .chunk
          .preliminary_filename
          .clone()
          .expect("should have preliminary filename"),
      }],
      warnings: std::mem::take(&mut ctx.warnings),
    }))
  }
}