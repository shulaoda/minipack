use minipack_common::ModuleLoaderMsg;
use minipack_fs::OsFileSystem;
use oxc::transformer::ReplaceGlobalDefinesConfig;

use crate::types::{SharedOptions, SharedResolver};

/// Used to store common data shared between all tasks.
pub struct TaskContext {
  pub options: SharedOptions,
  pub tx: tokio::sync::mpsc::Sender<ModuleLoaderMsg>,
  pub resolver: SharedResolver,
  pub fs: OsFileSystem,
  pub meta: TaskContextMeta,
}

pub struct TaskContextMeta {
  pub replace_global_define_config: Option<ReplaceGlobalDefinesConfig>,
}
