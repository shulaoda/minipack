use std::path::PathBuf;

use clap::Args;

use crate::types::{output_format::OutputFormat, platform::Platform};

#[derive(Args)]
pub struct InputArgs {
  /// Entry file(s)
  #[clap(long, action = clap::ArgAction::Append)]
  pub input: Option<Vec<PathBuf>>,

  /// Bundler platform environment
  #[clap(long, short, long)]
  pub platform: Option<Platform>,
}

#[derive(Args)]
pub struct OutputArgs {
  /// Output directory
  #[clap(long, short = 'd')]
  pub dir: Option<String>,

  /// Output module format
  #[clap(long)]
  pub format: Option<OutputFormat>,

  /// Output entry files, e.g. [name]-[hash].js
  #[clap(long)]
  pub entry_filenames: Option<String>,

  /// Output chunk files, e.g. [name]-[hash].js
  #[clap(long)]
  pub chunk_filenames: Option<String>,
}

#[derive(Args)]
pub struct EnhanceArgs {
  /// Minify the output bundle
  #[clap(long, short = 'm')]
  pub minify: bool,

  /// Suppress bundling logs
  #[clap(long, short = 's')]
  pub silent: bool,
}
