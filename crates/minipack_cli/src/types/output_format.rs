use clap::ValueEnum;

#[derive(PartialEq, Eq, Clone, ValueEnum)]
#[clap(rename_all = "lower")]
pub enum OutputFormat {
  Esm,
  Cjs,
}

impl From<OutputFormat> for minipack::OutputFormat {
  fn from(value: OutputFormat) -> Self {
    match value {
      OutputFormat::Esm => minipack::OutputFormat::Esm,
      OutputFormat::Cjs => minipack::OutputFormat::Cjs,
    }
  }
}
