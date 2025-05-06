use arcstr::ArcStr;

#[derive(Debug, Clone)]
pub struct OutputChunk {
  pub filename: ArcStr,
  pub content: String,
}

impl OutputChunk {
  pub fn filename(&self) -> &str {
    &self.filename
  }

  pub fn content_as_bytes(&self) -> &[u8] {
    self.content.as_bytes()
  }
}
