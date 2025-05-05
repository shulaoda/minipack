pub mod filename_template;
pub mod input_item;
pub mod module_type;
pub mod normalized_bundler_options;
pub mod output_format;
pub mod platform;

use std::path::PathBuf;

use crate::{InputItem, OutputFormat, Platform};

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
  pub entry_filenames: Option<String>,
  pub chunk_filenames: Option<String>,

  // --- Enhance
  pub minify: Option<bool>,
}
