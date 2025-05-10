use std::path::PathBuf;

use crate::{InputItem, OutputFormat, Platform};

#[derive(Debug)]
pub struct NormalizedBundlerOptions {
  // --- Input
  pub cwd: PathBuf,
  pub input: Vec<InputItem>,
  pub platform: Platform,

  // --- Output
  pub dir: String,
  pub format: OutputFormat,
  pub entry_filenames: String,
  pub chunk_filenames: String,

  // --- Enhance
  pub minify: bool,
}

impl NormalizedBundlerOptions {
  #[inline]
  pub fn is_esm_format_with_node_platform(&self) -> bool {
    matches!(self.format, OutputFormat::Esm) && matches!(self.platform, Platform::Node)
  }
}
