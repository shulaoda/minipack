use std::path::PathBuf;

use crate::{ESTarget, EsModuleFlag, InputItem, OutputExports, OutputFormat, Platform};

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
  pub css_entry_filenames: String,
  pub css_chunk_filenames: String,
  pub dir: String,
  pub file: Option<String>,
  pub format: OutputFormat,
  pub exports: OutputExports,
  pub es_module: EsModuleFlag,
  pub minify: bool,

  // --- Resolve
  pub target: ESTarget,
  pub inline_dynamic_imports: bool,
}

impl NormalizedBundlerOptions {
  pub fn is_esm_format_with_node_platform(&self) -> bool {
    matches!(self.format, OutputFormat::Esm) && matches!(self.platform, Platform::Node)
  }
}
