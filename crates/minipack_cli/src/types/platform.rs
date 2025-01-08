use clap::ValueEnum;

#[derive(PartialEq, Eq, Clone, ValueEnum)]
#[clap(rename_all = "lower")]
pub enum Platform {
  Node,
  Browser,
  Neutral,
}

impl From<Platform> for minipack::Platform {
  fn from(value: Platform) -> Self {
    match value {
      Platform::Node => minipack::Platform::Node,
      Platform::Browser => minipack::Platform::Browser,
      Platform::Neutral => minipack::Platform::Neutral,
    }
  }
}
