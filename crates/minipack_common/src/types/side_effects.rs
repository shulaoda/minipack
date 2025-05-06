#[derive(Debug, Copy, Clone)]
pub enum DeterminedSideEffects {
  Analyzed(bool),
  NoTreeshake,
}

impl DeterminedSideEffects {
  pub fn has_side_effects(&self) -> bool {
    match self {
      Self::Analyzed(v) => *v,
      Self::NoTreeshake => true,
    }
  }
}
