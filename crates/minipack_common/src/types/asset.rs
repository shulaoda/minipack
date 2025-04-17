use std::path::PathBuf;

use arcstr::ArcStr;

use crate::{ChunkIdx, PreliminaryFilename};

#[derive(Debug)]
/// Assets is final output of the bundling process. Inputs -> Modules -> Chunks -> Assets
pub struct Asset {
  pub origin_chunk: ChunkIdx,
  pub content: String,
  pub meta: Option<ArcStr>,
  pub augment_chunk_hash: Option<String>,
  pub file_dir: PathBuf,
  pub preliminary_filename: PreliminaryFilename,
  pub filename: String,
}
