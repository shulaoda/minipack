use crate::{OutputAsset, PreliminaryFilename};

#[derive(Debug)]
pub struct InstantiatedChunk {
  pub content: String,
  pub preliminary_filename: PreliminaryFilename,
}

impl InstantiatedChunk {
  pub fn finalize(self, filename: String) -> OutputAsset {
    OutputAsset { filename, content: self.content }
  }
}
