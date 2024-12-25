use std::sync::Arc;

use crate::{
  stages::{
    generate::GenerateStage,
    link::LinkStage,
    scan::{ScanStage, ScanStageOutput},
  },
  types::{bundle_output::BundleOutput, SharedOptions, SharedResolver},
  utils::normalize_options::{normalize_options, NormalizeOptionsReturn},
};

use minipack_common::BundlerOptions;
use minipack_error::BuildResult;
use minipack_fs::OsFileSystem;
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

  pub async fn build(&mut self, is_write: bool) -> BuildResult<BundleOutput> {
    if self.closed {
      return Err(anyhow::anyhow!(
        "Bundle is already closed, no more calls to 'generate' or 'write' are allowed."
      ))?;
    }

    let scan_stage_output = self.scan().await?;

    let mut link_stage_output = LinkStage::new(scan_stage_output, &self.options).link();

    let bundle_output =
      GenerateStage::new(&mut link_stage_output, &self.options).generate().await?;

    Ok(bundle_output)
  }

  pub async fn scan(&self) -> BuildResult<ScanStageOutput> {
    ScanStage::new(self.fs, self.options.clone(), self.resolver.clone()).scan().await
  }
}

#[test]
fn test_rust_syntax_errors() {}
