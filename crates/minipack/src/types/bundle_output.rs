use minipack_common::Output;

#[derive(Default)]
pub struct BundleOutput {
  pub assets: Vec<Output>,
  pub warnings: Vec<anyhow::Error>,
}
