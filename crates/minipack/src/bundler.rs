use std::sync::Arc;

use crate::{
  stages::scan::{ScanStage, ScanStageOutput},
  types::{SharedOptions, SharedResolver},
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

  pub async fn build(&mut self, is_write: bool) -> BuildResult<()> {
    let scan_stage = ScanStage::new(self.fs, self.options.clone(), self.resolver.clone());

    Ok(())
  }

  pub async fn scan(&self) -> BuildResult<ScanStageOutput> {
    let mut scan_stage = ScanStage::new(self.fs, self.options.clone(), self.resolver.clone());

    scan_stage.scan().await
  }
}

#[test]
fn test_rust_syntax_errors() {}
