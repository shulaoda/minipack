use arcstr::ArcStr;

use crate::{OutputChunk, StrOrBytes};

#[derive(Debug, Clone)]
pub struct OutputAsset {
  pub name: Option<String>,
  pub filename: ArcStr,
  pub original_file_name: Option<String>,
  pub source: StrOrBytes,
}

#[derive(Debug, Clone)]
pub enum Output {
  Chunk(Box<OutputChunk>),
  Asset(Box<OutputAsset>),
}

impl Output {
  pub fn filename(&self) -> &str {
    match self {
      Self::Chunk(chunk) => &chunk.filename,
      Self::Asset(asset) => &asset.filename,
    }
  }

  pub fn content_as_bytes(&self) -> &[u8] {
    match self {
      Self::Chunk(chunk) => chunk.code.as_bytes(),
      Self::Asset(asset) => asset.source.as_bytes(),
    }
  }
}