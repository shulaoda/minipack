use arcstr::ArcStr;
use minipack_common::Output;

#[derive(Default)]
pub struct BundleOutput {
  pub assets: Vec<Output>,
  pub watch_files: Vec<ArcStr>,
  pub warnings: Vec<anyhow::Error>,
}
