use std::fmt::Debug;

use memchr::memmem;

#[inline]
fn lines_count(str: &str) -> u32 {
  u32::try_from(memmem::find_iter(str.as_bytes(), "\n").count()).unwrap()
}

#[test]
fn test() {
  assert_eq!(lines_count("a\nb\nc"), 2);
  assert_eq!(lines_count("a\nb\nc\n"), 3);
  assert_eq!(lines_count("a"), 0);
}

pub trait Source {
  fn content(&self) -> &str;
  fn lines_count(&self) -> u32 {
    lines_count(self.content())
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

#[derive(Debug)]
pub struct SourceMapSource {
  content: String,
  pre_computed_lines_count: Option<u32>,
}

impl SourceMapSource {
  pub fn new(content: String) -> Self {
    Self { content, pre_computed_lines_count: None }
  }
}

impl Source for SourceMapSource {
  fn content(&self) -> &str {
    &self.content
  }

  fn lines_count(&self) -> u32 {
    self.pre_computed_lines_count.unwrap_or_else(|| lines_count(&self.content))
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
