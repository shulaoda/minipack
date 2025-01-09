use std::sync::Arc;

use minipack_common::{NormalModule, Source};
use minipack_utils::concat_string;

pub fn render_ecma_module(
  module: &NormalModule,
  code: String,
) -> Option<Arc<[Box<dyn Source + Send + Sync>]>> {
  if code.is_empty() {
    return None;
  }

  let sources: Vec<Box<dyn Source + Send + Sync>> = vec![
    Box::new(concat_string!("//#region ", module.debug_id)),
    Box::new(code),
    Box::new("//#endregion"),
  ];

  Some(Arc::from(sources.into_boxed_slice()))
}
