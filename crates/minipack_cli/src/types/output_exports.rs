use clap::ValueEnum;

#[derive(PartialEq, Eq, Clone, ValueEnum)]
pub enum OutputExports {
  Auto,
  Default,
  Named,
  None,
}

impl From<OutputExports> for minipack::OutputExports {
  fn from(value: OutputExports) -> Self {
    match value {
      OutputExports::Auto => minipack::OutputExports::Auto,
      OutputExports::Default => minipack::OutputExports::Default,
      OutputExports::Named => minipack::OutputExports::Named,
      OutputExports::None => minipack::OutputExports::None,
    }
  }
}
