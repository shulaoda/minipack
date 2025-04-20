pub mod ast_scanner;
pub mod loaders;
pub mod module_loader;
pub mod task_context;

mod module_task;
mod runtime_module_task;

use module_loader::{ModuleLoader, ModuleLoaderOutput};

use arcstr::ArcStr;
use minipack_common::ResolvedId;
use minipack_error::BuildResult;
use minipack_fs::OsFileSystem;

use crate::{
  types::{SharedOptions, SharedResolver},
  utils::resolve_id::resolve_id,
};

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
    if self.options.input.is_empty() {
      return Err(anyhow::anyhow!("You must supply options.input to rolldown"))?;
    }

    let module_loader = ModuleLoader::new(self.fs, self.options.clone(), self.resolver.clone())?;
    let user_entries: Vec<(Option<ArcStr>, ResolvedId)> = self
      .options
      .input
      .iter()
      .map(|input_item| {
        resolve_id(&self.resolver, &input_item.import, None, true)
          .map_err(|e| anyhow::anyhow!("ResolveError: {:?}", e))
          .and_then(|resolved_id| {
            if resolved_id.is_external {
              Err(anyhow::anyhow!(
                "Failed to resolve {:?} - entry can't be external",
                resolved_id.id.to_string()
              ))
            } else {
              Ok((input_item.name.as_ref().map(ArcStr::from), resolved_id))
            }
          })
      })
      .collect::<Result<_, anyhow::Error>>()?;

    module_loader.fetch_all_modules(user_entries).await
  }
}
