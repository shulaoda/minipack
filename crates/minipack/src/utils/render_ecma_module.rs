use std::sync::Arc;

use minipack_common::{ModuleRenderOutput, NormalModule, NormalizedBundlerOptions};
use minipack_sourcemap::Source;
use minipack_utils::concat_string;

pub fn render_ecma_module(
  module: &NormalModule,
  options: &NormalizedBundlerOptions,
  render_output: ModuleRenderOutput,
) -> Option<Arc<[Box<dyn Source + Send + Sync>]>> {
  if render_output.code.is_empty() {
    None
  } else {
    let mut sources: Vec<Box<dyn Source + Send + Sync>> = vec![];

    sources.push(Box::new(concat_string!("//#region ", module.debug_id)));

    sources.push(Box::new(render_output.code));

    sources.push(Box::new("//#endregion"));

    Some(Arc::from(sources.into_boxed_slice()))
  }
}
