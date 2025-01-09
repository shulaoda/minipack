#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModuleType {
  Js,
  Jsx,
  Ts,
  Tsx,
  Json,
  Text,
  Base64,
  Dataurl,
  Binary,
  Empty,
  Css,
  Asset,
  Custom(String),
}
