#[derive(Debug, Clone, Copy)]
pub enum OutputExports {
  Auto,
  Default,
  Named,
  None,
}

impl Default for OutputExports {
  fn default() -> Self {
    Self::Auto
  }
}
