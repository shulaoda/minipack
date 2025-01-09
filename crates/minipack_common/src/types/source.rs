use memchr::memmem;

pub trait Source {
  fn content(&self) -> &str;
  fn lines_count(&self) -> u32 {
    u32::try_from(memmem::find_iter(self.content().as_bytes(), "\n").count()).unwrap()
  }
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

  fn lines_count(&self) -> u32 {
    self.as_ref().lines_count()
  }
}
