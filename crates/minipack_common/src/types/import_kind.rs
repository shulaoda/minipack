#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum ImportKind {
  Import,
  DynamicImport,
}

impl ImportKind {
  #[inline]
  pub fn is_static(&self) -> bool {
    matches!(self, Self::Import)
  }
}
