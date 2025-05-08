use minipack_common::OutputAsset;

#[derive(Default)]
pub struct BundleOutput {
  pub assets: Vec<OutputAsset>,
  pub warnings: Vec<anyhow::Error>,
}
