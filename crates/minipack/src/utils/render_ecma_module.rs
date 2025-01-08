use std::sync::Arc;

use minipack_common::{ModuleRenderOutput, NormalModule};
use minipack_sourcemap::Source;
use minipack_utils::concat_string;

pub fn render_ecma_module(
  module: &NormalModule,
  render_output: ModuleRenderOutput,
) -> Option<Arc<[Box<dyn Source + Send + Sync>]>> {
  if render_output.code.is_empty() {
    return None;
  }

  let sources: Vec<Box<dyn Source + Send + Sync>> = vec![
    Box::new(concat_string!("//#region ", module.debug_id)),
    Box::new(render_output.code),
    Box::new("//#endregion"),
  ];

  Some(Arc::from(sources.into_boxed_slice()))
}
