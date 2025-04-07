#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModuleType {
  Js,
  Jsx,
  Ts,
  Tsx,
  Json,
  Empty,
  Custom(String),
}
