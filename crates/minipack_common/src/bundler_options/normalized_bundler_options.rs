use std::path::PathBuf;

use crate::{InputItem, OutputFormat, Platform};

#[allow(clippy::struct_excessive_bools)] // Using raw booleans is more clear in this case
#[derive(Debug)]
pub struct NormalizedBundlerOptions {
  // --- Input
  pub input: Vec<InputItem>,
  pub cwd: PathBuf,
  pub platform: Platform,
  pub shim_missing_exports: bool,

  // --- Output
  pub name: Option<String>,
  pub entry_filenames: String,
  pub chunk_filenames: String,
  pub asset_filenames: String,
  pub dir: String,
  pub file: Option<String>,
  pub format: OutputFormat,
}

impl NormalizedBundlerOptions {
  pub fn is_esm_format_with_node_platform(&self) -> bool {
    matches!(self.format, OutputFormat::Esm) && matches!(self.platform, Platform::Node)
  }
}
