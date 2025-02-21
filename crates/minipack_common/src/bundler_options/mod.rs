pub mod es_target;
pub mod filename_template;
pub mod input_item;
pub mod module_type;
pub mod normalized_bundler_options;
pub mod output_exports;
pub mod output_format;
pub mod platform;
pub mod resolve_options;

use std::path::PathBuf;

use crate::{ESTarget, InputItem, OutputExports, OutputFormat, Platform, ResolveOptions};

#[derive(Default, Debug, Clone)]
pub struct BundlerOptions {
  // --- Input
  pub cwd: Option<PathBuf>,
  pub input: Option<Vec<InputItem>>,
  pub platform: Option<Platform>,

  // --- Output
  pub dir: Option<String>,
  pub file: Option<String>,
  pub format: Option<OutputFormat>,
  pub exports: Option<OutputExports>,
  pub entry_filenames: Option<String>,
  pub chunk_filenames: Option<String>,
  pub asset_filenames: Option<String>,
  pub css_entry_filenames: Option<String>,
  pub css_chunk_filenames: Option<String>,

  // --- Enhance
  pub minify: Option<bool>,
  pub target: Option<ESTarget>,
  pub shim_missing_exports: Option<bool>,

  // --- Resolve
  pub resolve: Option<ResolveOptions>,
}
