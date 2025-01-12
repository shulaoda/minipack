use crate::RollupRenderedChunk;

#[derive(Debug)]

pub enum InstantiationKind {
  Ecma(Box<RollupRenderedChunk>),
  // Using Variant `None` instead of `Option<AssetMeta>` to make it friendly to use pattern matching.
  None,
}

impl From<RollupRenderedChunk> for InstantiationKind {
  fn from(rendered_chunk: RollupRenderedChunk) -> Self {
    InstantiationKind::Ecma(Box::new(rendered_chunk))
  }
}
