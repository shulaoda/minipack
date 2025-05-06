use std::path::PathBuf;

use clap::Args;

use crate::types::{output_format::OutputFormat, platform::Platform};

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

  #[clap(long)]
  pub format: Option<OutputFormat>,

  #[clap(long)]
  pub entry_filenames: Option<String>,

  #[clap(long)]
  pub chunk_filenames: Option<String>,
}

#[derive(Args)]
pub struct EnhanceArgs {
  #[clap(long, short = 'm')]
  pub minify: Option<bool>,

  #[clap(long, short = 's')]
  pub silent: bool,
}
