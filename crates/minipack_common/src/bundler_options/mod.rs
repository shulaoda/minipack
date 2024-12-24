pub mod es_module_flag;
pub mod es_target;
pub mod filename_template;
pub mod input_item;
pub mod jsx;
pub mod module_type;
pub mod normalized_bundler_options;
pub mod output_exports;
pub mod output_format;
pub mod platform;
pub mod resolve_options;

use std::path::PathBuf;

use crate::{
  ESTarget, EsModuleFlag, InputItem, OutputExports, OutputFormat, Platform, ResolveOptions,
};

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
  pub exports: Option<OutputExports>,
  pub es_module: Option<EsModuleFlag>,

  // --- Resolve
  pub target: Option<ESTarget>,
  pub resolve: Option<ResolveOptions>,
  pub inline_dynamic_imports: Option<bool>,
}
