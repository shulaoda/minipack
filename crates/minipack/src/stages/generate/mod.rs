use arcstr::ArcStr;
use minipack_common::ChunkIdx;
use minipack_error::BuildResult;
use rustc_hash::FxHashMap;

use crate::{
  graph::Graph,
  types::{bundle_output::BundleOutput, SharedOptions},
};

use super::link::LinkStageOutput;

pub struct GenerateStage<'a> {
  link_output: &'a mut LinkStageOutput,
  options: &'a SharedOptions,
}

impl<'a> GenerateStage<'a> {
  pub fn new(link_output: &'a mut LinkStageOutput, options: &'a SharedOptions) -> Self {
    Self { link_output, options }
  }

  pub async fn generate(&mut self) -> BuildResult<BundleOutput> {
    todo!()
  }

  /// Notices:
  /// - Should generate filenames that are stable cross builds and os.
  async fn generate_chunk_name_and_preliminary_filenames(
    &self,
    graph: &mut Graph,
  ) -> BuildResult<FxHashMap<ChunkIdx, ArcStr>> {
    todo!()
  }

  pub fn patch_asset_modules(&mut self, chunk_graph: &Graph) {
    todo!()
  }
}
