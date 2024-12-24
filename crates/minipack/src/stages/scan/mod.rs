use arcstr::ArcStr;
use minipack_common::{ImportKind, ResolvedId};
use minipack_error::BuildResult;
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
      Err(vec![anyhow::anyhow!("You must supply options.input to rolldown")])?;
    }

    let user_entries = self.resolve_user_defined_entries().await?;

    let module_loader = ModuleLoader::new(self.fs, self.options.clone(), self.resolver.clone())?;
    let output = module_loader.fetch_all_modules(user_entries).await?;

    Ok(output)
  }

  async fn resolve_user_defined_entries(
    &mut self,
  ) -> BuildResult<Vec<(Option<ArcStr>, ResolvedId)>> {
    let resolver = &self.resolver;

    let resolved_ids = self.options.input.iter().map(|input_item| {
      let resolved = resolve_id(resolver, &input_item.import, None, ImportKind::Import, true);

      resolved.map(|info| ((input_item.name.clone().map(ArcStr::from)), info))
    });

    let mut ret = Vec::with_capacity(self.options.input.len());

    let mut errors = vec![];

    for resolve_id in resolved_ids {
      match resolve_id {
        Ok(item) => {
          if item.1.is_external {
            errors.push(anyhow::anyhow!(
              "Failed to resolve {:?} - entry can't be external",
              item.1.id.to_string()
            ));
            continue;
          }
          ret.push(item);
        }
        Err(e) => errors.push(anyhow::anyhow!("ResolveError: {:?}", e)),
      }
    }

    if !errors.is_empty() {
      Err(errors)?;
    }

    Ok(ret)
  }
}
