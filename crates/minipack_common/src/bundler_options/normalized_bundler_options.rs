use std::path::PathBuf;

use crate::{ESTarget, InputItem, OutputExports, OutputFormat, Platform};

// Using raw booleans is more clear in this case
#[derive(Debug)]
pub struct NormalizedBundlerOptions {
  // --- Input
  pub cwd: PathBuf,
  pub input: Vec<InputItem>,
  pub platform: Platform,

  // --- Output
  pub dir: String,
  pub file: Option<String>,
  pub format: OutputFormat,
  pub exports: OutputExports,
  pub entry_filenames: String,
  pub chunk_filenames: String,
  pub asset_filenames: String,
  pub css_entry_filenames: String,
  pub css_chunk_filenames: String,

  // --- Enhance
  pub minify: bool,
  pub target: ESTarget,
  pub shim_missing_exports: bool,
  pub inline_dynamic_imports: bool,
}

impl NormalizedBundlerOptions {
  pub fn is_esm_format_with_node_platform(&self) -> bool {
    matches!(self.format, OutputFormat::Esm) && matches!(self.platform, Platform::Node)
  }
}
