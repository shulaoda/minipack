use crate::{Asset, PreliminaryFilename};

#[derive(Debug)]
pub struct InstantiatedChunk {
  pub content: String,
  pub preliminary_filename: PreliminaryFilename,
}

impl InstantiatedChunk {
  pub fn finalize(self, filename: String) -> Asset {
    Asset { filename, content: self.content }
  }
}
