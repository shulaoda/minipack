use minipack_common::OutputChunk;

#[derive(Default)]
pub struct BundleOutput {
  pub assets: Vec<OutputChunk>,
  pub warnings: Vec<anyhow::Error>,
}
