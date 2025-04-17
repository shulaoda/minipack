use std::borrow::Cow;

#[derive(Debug, Default, Clone)]
pub struct InputItem {
  pub name: Option<String>,
  pub import: String,
}

impl From<&str> for InputItem {
  fn from(value: &str) -> Self {
    Self { name: None, import: value.to_string() }
  }
}

impl From<Cow<'_, str>> for InputItem {
  fn from(value: Cow<'_, str>) -> Self {
    Self { name: None, import: value.to_string() }
  }
}
