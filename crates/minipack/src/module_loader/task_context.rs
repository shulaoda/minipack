use minipack_common::ModuleLoaderMsg;
use minipack_fs::OsFileSystem;

use crate::types::{SharedOptions, SharedResolver};

pub struct TaskContext {
  pub fs: OsFileSystem,
  pub options: SharedOptions,
  pub resolver: SharedResolver,
  pub tx: tokio::sync::mpsc::Sender<ModuleLoaderMsg>,
}
