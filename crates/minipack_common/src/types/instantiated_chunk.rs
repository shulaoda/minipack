use std::path::PathBuf;

use arcstr::ArcStr;

use crate::{Asset, ChunkIdx, PreliminaryFilename};

/// `InstantiatedChunk`s are derived from `Chunk`s.
#[derive(Debug)]
pub struct InstantiatedChunk {
  pub origin_chunk: ChunkIdx,
  pub content: String,
  pub kind: Option<ArcStr>,
  pub augment_chunk_hash: Option<String>,
  pub file_dir: PathBuf,
  pub preliminary_filename: PreliminaryFilename,
}

impl InstantiatedChunk {
  pub fn finalize(self, filename: String) -> Asset {
    Asset { filename, content: self.content }
  }
}
