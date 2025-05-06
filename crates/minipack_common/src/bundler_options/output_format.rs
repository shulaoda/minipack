use std::fmt::Display;

#[derive(Debug, Default, Clone, Copy)]
pub enum OutputFormat {
  #[default]
  Esm,
  Cjs,
}

impl OutputFormat {
  #[inline]
  pub fn is_esm(&self) -> bool {
    matches!(self, Self::Esm)
  }
}

impl Display for OutputFormat {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      Self::Esm => write!(f, "esm"),
      Self::Cjs => write!(f, "cjs"),
    }
  }
}
