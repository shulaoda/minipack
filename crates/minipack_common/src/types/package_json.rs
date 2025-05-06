use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct PackageJson {
  pub path: PathBuf,
  pub r#type: Option<String>,
}

impl PackageJson {
  pub fn new(path: PathBuf) -> Self {
    Self { path, r#type: None }
  }

  #[must_use]
  pub fn with_type(mut self, value: Option<&str>) -> Self {
    self.r#type = value.map(ToString::to_string);
    self
  }

  pub fn r#type(&self) -> Option<&str> {
    self.r#type.as_deref()
  }
}
