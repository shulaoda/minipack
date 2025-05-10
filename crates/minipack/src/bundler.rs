use std::sync::Arc;

use minipack_common::BundlerOptions;
use minipack_error::BuildResult;
use minipack_fs::{FileSystem, OsFileSystem};
use minipack_resolver::Resolver;

use crate::{
  generate_stage::GenerateStage,
  link_stage::{LinkStage, LinkStageOutput},
  scan_stage::{ScanStage, ScanStageOutput},
  types::{SharedOptions, SharedResolver, bundle_output::BundleOutput},
};

pub struct Bundler {
  pub(crate) fs: OsFileSystem,
  pub(crate) options: SharedOptions,
  pub(crate) resolver: SharedResolver,
}

impl Bundler {
  pub fn new(options: BundlerOptions) -> Self {
    let options = crate::utils::normalize_bundler_options(options);
    let resolver = Arc::new(Resolver::new(options.platform, options.cwd.clone(), OsFileSystem));
    Bundler { fs: OsFileSystem, options, resolver }
  }

  pub async fn build(&mut self, is_write: bool) -> BuildResult<BundleOutput> {
    if self.options.input.is_empty() {
      return Err(anyhow::anyhow!("You must supply `options.input`."))?;
    }

    let scan_stage_output = self.scan().await?;
    let link_stage_output = self.link(scan_stage_output).await;
    let generate_stage_output = self.generate(link_stage_output).await?;

    if is_write {
      let dist = self.options.cwd.join(&self.options.dir);
      self.fs.create_dir_all(&dist).map_err(|err| {
        anyhow::anyhow!("Couldn't create output directory: {:?}", dist).context(err)
      })?;
      for chunk in &generate_stage_output.assets {
        let filename = dist.join(&chunk.filename);
        self
          .fs
          .write(&filename, chunk.content.as_bytes())
          .map_err(|err| anyhow::anyhow!("Failed to write file {filename:?}").context(err))?;
      }
    }

    Ok(generate_stage_output)
  }

  #[inline]
  async fn scan(&self) -> BuildResult<ScanStageOutput> {
    ScanStage::new(self.fs, self.options.clone(), self.resolver.clone()).scan().await
  }

  #[inline]
  async fn link(&self, scan_stage_output: ScanStageOutput) -> LinkStageOutput {
    LinkStage::new(scan_stage_output, self.options.clone()).link()
  }

  #[inline]
  async fn generate(&self, link_stage_output: LinkStageOutput) -> BuildResult<BundleOutput> {
    GenerateStage::new(link_stage_output, self.options.clone()).generate().await
  }
}

#[test]
fn test_rust_syntax_errors() {}
