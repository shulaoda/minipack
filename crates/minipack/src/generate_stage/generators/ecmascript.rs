use std::sync::Arc;

use minipack_common::{InstantiatedChunk, ModuleIdx, OutputFormat, Source};
use minipack_error::BuildResult;
use minipack_utils::rayon::{IndexedParallelIterator, IntoParallelRefIterator, ParallelIterator};

use crate::types::generator::{GenerateContext, GenerateOutput};

use super::formats::{cjs::render_cjs, esm::render_esm};

pub struct RenderedModuleSource {
  pub module_idx: ModuleIdx,
  pub sources: Option<Arc<[Box<dyn Source + Send + Sync>]>>,
}

impl RenderedModuleSource {
  pub fn new(module_idx: ModuleIdx, sources: Option<Arc<[Box<dyn Source + Send + Sync>]>>) -> Self {
    Self { module_idx, sources }
  }
}
pub struct EcmaGenerator;

impl EcmaGenerator {
  pub async fn instantiate_chunk(ctx: &mut GenerateContext<'_>) -> BuildResult<GenerateOutput> {
    let module_id_to_codegen_ret = std::mem::take(&mut ctx.module_id_to_codegen_ret);
    let rendered_module = ctx
      .chunk
      .modules
      .par_iter()
      .zip(module_id_to_codegen_ret)
      .filter_map(|(id, code)| {
        ctx.link_output.module_table[*id].as_normal().map(|m| (m, code.unwrap()))
      })
      .map(|(m, code)| {
        RenderedModuleSource::new(
          m.idx,
          (!code.is_empty()).then(|| Arc::from([Box::new(code) as Box<dyn Source + Send + Sync>])),
        )
      })
      .collect::<Vec<_>>();

    let source_joiner = match ctx.options.format {
      OutputFormat::Esm => render_esm(ctx, &rendered_module),
      OutputFormat::Cjs => render_cjs(ctx, &rendered_module)?,
    };

    let content = source_joiner.join();
    let preliminary_filename =
      ctx.chunk.preliminary_filename.clone().expect("should have preliminary filename");

    Ok(GenerateOutput {
      chunks: vec![InstantiatedChunk { content, preliminary_filename }],
      warnings: std::mem::take(&mut ctx.warnings),
    })
  }
}
