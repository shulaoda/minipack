use std::fmt::Display;

#[derive(Debug, Clone, Copy)]
pub enum OutputFormat {
  Esm,
  Cjs,
}

impl OutputFormat {
  #[inline]
  pub fn requires_scope_hoisting(&self) -> bool {
    matches!(self, Self::Esm | Self::Cjs)
  }

  #[inline]
  pub fn should_call_runtime_require(&self) -> bool {
    !matches!(self, Self::Cjs)
  }

  #[inline]
  pub fn keep_esm_import_export_syntax(&self) -> bool {
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
