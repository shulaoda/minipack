use std::sync::Arc;

use crate::{
  stages::{generate::GenerateStage, link::LinkStage, scan::ScanStage},
  types::{bundle_output::BundleOutput, SharedOptions, SharedResolver},
  utils::normalize_options::{normalize_options, NormalizeOptionsReturn},
};

use minipack_common::BundlerOptions;
use minipack_error::BuildResult;
use minipack_fs::{FileSystem, OsFileSystem};
use minipack_resolver::Resolver;

pub struct Bundler {
  pub closed: bool,
  pub(crate) fs: OsFileSystem,
  pub(crate) options: SharedOptions,
  pub(crate) resolver: SharedResolver,
}

impl Bundler {
  pub fn new(options: BundlerOptions) -> Self {
    let NormalizeOptionsReturn { options, resolve_options } = normalize_options(options);

    let resolver: SharedResolver =
      Resolver::new(resolve_options, options.platform, options.cwd.clone(), OsFileSystem).into();

    Bundler { closed: false, fs: OsFileSystem, options: Arc::new(options), resolver }
  }

  pub async fn write(&mut self) -> BuildResult<BundleOutput> {
    self.build(true).await
  }

  pub async fn generate(&mut self) -> BuildResult<BundleOutput> {
    self.build(false).await
  }

  async fn build(&mut self, is_write: bool) -> BuildResult<BundleOutput> {
    if self.closed {
      return Err(anyhow::anyhow!(
        "Bundle is already closed, no more calls to 'generate' or 'write' are allowed."
      ))?;
    }

    let scan_stage_output =
      ScanStage::new(self.fs, self.options.clone(), self.resolver.clone()).scan().await?;

    let mut link_stage_output = LinkStage::new(scan_stage_output, &self.options).link();

    let bundle_output =
      GenerateStage::new(&mut link_stage_output, &self.options).generate().await?;

    if is_write {
      let dist_dir = self.options.cwd.join(&self.options.dir);

      self.fs.create_dir_all(&dist_dir).map_err(|err| {
        anyhow::anyhow!("Could not create directory for output chunks: {:?}", dist_dir).context(err)
      })?;

      for chunk in &bundle_output.assets {
        let dest = dist_dir.join(chunk.filename());
        if let Some(p) = dest.parent() {
          if !self.fs.exists(p) {
            self.fs.create_dir_all(p).unwrap();
          }
        };
        self
          .fs
          .write(&dest, chunk.content_as_bytes())
          .map_err(|err| anyhow::anyhow!("Failed to write file in {:?}", dest).context(err))?;
      }
    }

    Ok(bundle_output)
  }
}

#[test]
fn test_rust_syntax_errors() {}
