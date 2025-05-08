#[derive(Debug, Clone)]
pub struct OutputAsset {
  pub filename: String,
  pub content: String,
}

impl OutputAsset {
  pub fn filename(&self) -> &str {
    &self.filename
  }

  pub fn content_as_bytes(&self) -> &[u8] {
    self.content.as_bytes()
  }
}
