#[derive(Debug, Default, Clone, Copy)]
pub enum OutputExports {
  #[default]
  Auto,
  Default,
  Named,
  None,
}
