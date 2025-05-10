pub mod ast_scanner;
pub mod loaders;
pub mod module_loader;

mod module_task;
mod runtime_module_task;

use module_loader::{ModuleLoader, ModuleLoaderOutput};

use arcstr::ArcStr;
use minipack_error::BuildResult;
use minipack_fs::OsFileSystem;

use crate::types::{SharedOptions, SharedResolver};

pub type ScanStageOutput = ModuleLoaderOutput;

pub struct ScanStage {
  fs: OsFileSystem,
  options: SharedOptions,
  resolver: SharedResolver,
}

impl ScanStage {
  pub fn new(fs: OsFileSystem, options: SharedOptions, resolver: SharedResolver) -> Self {
    Self { fs, options, resolver }
  }

  pub async fn scan(&mut self) -> BuildResult<ScanStageOutput> {
    let module_loader = ModuleLoader::new(self.fs, self.options.clone(), self.resolver.clone())?;
    let mut user_defined_entries = Vec::with_capacity(self.options.input.len());
    for input_item in &self.options.input {
      user_defined_entries.push(
        self
          .resolver
          .resolve_id(&input_item.import, None, true)
          .map_err(|e| anyhow::anyhow!("ResolveError: {:?}", e))
          .and_then(|resolved_id| {
            if resolved_id.is_external {
              Err(anyhow::anyhow!(
                "Failed to resolve {:?} - entry can't be external",
                resolved_id.id
              ))
            } else {
              Ok((input_item.name.as_ref().map(ArcStr::from), resolved_id))
            }
          })?,
      );
    }
    module_loader.fetch_all_modules(user_defined_entries).await
  }
}
