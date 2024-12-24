pub mod es_target;
pub mod filename_template;
pub mod input_item;
pub mod jsx;
pub mod module_type;
pub mod normalized_bundler_options;
pub mod output_format;
pub mod platform;
pub mod resolve_options;

use std::path::PathBuf;

use crate::{ESTarget, InputItem, OutputFormat, Platform, ResolveOptions};

#[derive(Default, Debug, Clone)]
pub struct BundlerOptions {
  // --- Input
  pub input: Option<Vec<InputItem>>,
  pub cwd: Option<PathBuf>,
  pub platform: Option<Platform>,
  pub shim_missing_exports: Option<bool>,

  // --- Output
  pub name: Option<String>,
  pub entry_filenames: Option<String>,
  pub chunk_filenames: Option<String>,
  pub asset_filenames: Option<String>,
  pub css_entry_filenames: Option<String>,
  pub css_chunk_filenames: Option<String>,
  pub dir: Option<String>,
  pub file: Option<String>,
  pub format: Option<OutputFormat>,
  pub inline_dynamic_imports: Option<bool>,

  // --- Resolve
  pub target: Option<ESTarget>,
  pub resolve: Option<ResolveOptions>,
}
