use memchr::memmem;

pub trait Source {
  fn content(&self) -> &str;
  fn lines_count(&self) -> u32 {
    let haystack = self.content().as_bytes();
    u32::try_from(memmem::find_iter(haystack, "\n").count()).unwrap()
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
}
