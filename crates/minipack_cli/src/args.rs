use std::path::PathBuf;

use clap::Args;

use crate::types::{
  es_target::ESTarget, output_exports::OutputExports, output_format::OutputFormat,
  platform::Platform,
};

#[derive(Args)]
pub struct InputArgs {
  #[clap(long)]
  pub cwd: Option<PathBuf>,

  #[clap(long, action = clap::ArgAction::Append)]
  pub input: Option<Vec<PathBuf>>,

  #[clap(long, short, long)]
  pub platform: Option<Platform>,
}

#[derive(Args)]
pub struct OutputArgs {
  #[clap(long, short = 'd')]
  pub dir: Option<String>,

  #[clap(long, short = 'o')]
  pub file: Option<String>,

  #[clap(long)]
  pub format: Option<OutputFormat>,

  #[clap(long)]
  pub exports: Option<OutputExports>,

  #[clap(long)]
  pub entry_filenames: Option<String>,

  #[clap(long)]
  pub chunk_filenames: Option<String>,

  #[clap(long)]
  pub asset_filenames: Option<String>,

  #[clap(long)]
  pub css_entry_filenames: Option<String>,

  #[clap(long)]
  pub css_chunk_filenames: Option<String>,
}

#[derive(Args)]
pub struct EnhanceArgs {
  #[clap(long, short = 'm')]
  pub minify: Option<bool>,

  #[clap(long, default_missing_value = "esnext")]
  pub target: Option<ESTarget>,

  #[clap(long)]
  pub shim_missing_exports: Option<bool>,

  #[clap(long)]
  pub inline_dynamic_imports: Option<bool>,
}
