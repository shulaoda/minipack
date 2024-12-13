use minipack_common::ModuleLoaderMsg;
use minipack_fs::OsFileSystem;

use crate::types::{SharedOptions, SharedResolver};

/// Used to store common data shared between all tasks.
pub struct TaskContext {
  pub fs: OsFileSystem,
  pub options: SharedOptions,
  pub resolver: SharedResolver,
  pub tx: tokio::sync::mpsc::Sender<ModuleLoaderMsg>,
}
