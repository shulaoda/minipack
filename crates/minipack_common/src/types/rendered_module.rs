use std::{fmt::Debug, sync::Arc};

use super::{source::Source, source_joiner::SourceJoiner};

#[derive(Clone, Default)]
pub struct RenderedModule {
  inner: Option<Arc<[Box<dyn Source + Send + Sync>]>>,
}

impl Debug for RenderedModule {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("RenderedModule").finish()
  }
}

impl RenderedModule {
  pub fn new(sources: Option<Arc<[Box<dyn Source + Send + Sync>]>>) -> Self {
    Self { inner: sources }
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
