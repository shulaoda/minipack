use arcstr::ArcStr;
use minipack_common::{ImportKind, ResolvedId};
use minipack_error::{BuildError, BuildResult};
use minipack_fs::OsFileSystem;

use crate::{
  module_loader::{ModuleLoader, ModuleLoaderOutput},
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

    let user_entries = self.resolve_user_defined_entries().await?;

    let module_loader = ModuleLoader::new(self.fs, self.options.clone(), self.resolver.clone())?;
    let output = module_loader.fetch_all_modules(user_entries).await?;

    Ok(output)
  }

  async fn resolve_user_defined_entries(
    &mut self,
  ) -> BuildResult<Vec<(Option<ArcStr>, ResolvedId)>> {
    Ok(
      self
        .options
        .input
        .iter()
        .map(|input_item| {
          resolve_id(&self.resolver, &input_item.import, None, ImportKind::Import, true)
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
        .collect::<Result<Vec<(Option<ArcStr>, ResolvedId)>, anyhow::Error>>()?,
    )
  }
}
