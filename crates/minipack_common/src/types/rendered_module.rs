use std::sync::Arc;

use super::{source::Source, source_joiner::SourceJoiner};

pub struct RenderedModule {
  pub exec_order: u32,
  inner: Option<Arc<[Box<dyn Source + Send + Sync>]>>,
}

impl RenderedModule {
  pub fn new(inner: Option<Arc<[Box<dyn Source + Send + Sync>]>>, exec_order: u32) -> Self {
    Self { inner, exec_order }
  }

  pub fn code(&self) -> Option<String> {
    self.inner.as_ref().map(|sources| {
      let mut joiner = SourceJoiner::default();
      for source in sources.iter() {
        joiner.append_source(source);
      }
      joiner.join()
    })
  }
}
