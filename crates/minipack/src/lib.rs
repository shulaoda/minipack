mod bundler;
mod types;
mod utils;

use std::sync::Arc;

use minipack_fs::OsFileSystem;
use minipack_resolver::Resolver;

pub use crate::bundler::Bundler;
pub use minipack_common::*;

pub(crate) type SharedResolver = Arc<Resolver<OsFileSystem>>;
pub(crate) type SharedOptions = Arc<NormalizedBundlerOptions>;
