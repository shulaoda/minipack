use arcstr::ArcStr;

#[derive(Debug)]
pub struct ResolvedId {
  pub id: ArcStr,
  pub is_external: bool,
}
