use std::sync::Arc;

use crate::{
  types::bundle_output::BundleOutput,
  utils::normalize_options::{normalize_options, NormalizeOptionsReturn},
  SharedOptions, SharedResolver,
};

use minipack_common::BundlerOptions;
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

  async fn build(&mut self, is_write: bool) -> anyhow::Result<BundleOutput> {
    todo!()
  }
}
