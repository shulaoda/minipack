use std::ops::{Deref, DerefMut};

#[derive(Debug)]
pub struct BuildError(pub Vec<anyhow::Error>);

impl Deref for BuildError {
  type Target = Vec<anyhow::Error>;

  fn deref(&self) -> &Self::Target {
    &self.0
  }
}

impl DerefMut for BuildError {
  fn deref_mut(&mut self) -> &mut Self::Target {
    &mut self.0
  }
}

impl From<anyhow::Error> for BuildError {
  fn from(error: anyhow::Error) -> Self {
    Self(vec![error])
  }
}

impl From<Vec<anyhow::Error>> for BuildError {
  fn from(errors: Vec<anyhow::Error>) -> Self {
    Self(errors)
  }
}

pub type BuildResult<T> = anyhow::Result<T, BuildError>;
