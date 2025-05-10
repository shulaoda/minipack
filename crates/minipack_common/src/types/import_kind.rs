#[derive(Debug, Clone, Copy)]
pub enum ImportKind {
  Import,
  DynamicImport,
}

impl ImportKind {
  #[inline]
  pub fn is_static(&self) -> bool {
    matches!(self, Self::Import)
  }

  #[inline]
  pub fn is_dynamic(&self) -> bool {
    matches!(self, Self::DynamicImport)
  }
}
