#[derive(Debug)]
/// Assets is final output of the bundling process. Inputs -> Modules -> Chunks -> Assets
pub struct Asset {
  pub content: String,
  pub filename: String,
}
