pub trait Source {
  fn content(&self) -> &str;
}

impl Source for &str {
  fn content(&self) -> &str {
    self
  }
}

impl Source for String {
  fn content(&self) -> &str {
    self
  }
}

impl Source for &Box<dyn Source + Send + Sync> {
  fn content(&self) -> &str {
    self.as_ref().content()
  }
}
