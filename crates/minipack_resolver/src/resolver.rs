use std::{
  path::{Path, PathBuf},
  sync::Arc,
};

use arcstr::ArcStr;
use sugar_path::SugarPath;

use oxc_resolver::{FsCache, ResolveError, ResolveOptions as OxcResolverOptions, ResolverGeneric};

use minipack_common::Platform;
use minipack_fs::{FileSystem, OsFileSystem};

#[derive(Debug)]
pub struct Resolver<T: FileSystem + Default = OsFileSystem> {
  cwd: PathBuf,
  import_resolver: ResolverGeneric<FsCache<T>>,
}

impl<F: FileSystem + Default> Resolver<F> {
  #[allow(clippy::too_many_lines)]
  pub fn new(platform: Platform, cwd: PathBuf, fs: F) -> Self {
    let mut import_conditions = vec!["import".to_string(), "default".to_string()];

    match platform {
      Platform::Node => {
        import_conditions.push("node".to_string());
      }
      Platform::Browser => {
        import_conditions.push("browser".to_string());
      }
      Platform::Neutral => {}
    }

    let main_fields = match platform {
      Platform::Node => {
        vec!["main".to_string(), "module".to_string()]
      }
      Platform::Browser => vec!["browser".to_string(), "module".to_string(), "main".to_string()],
      Platform::Neutral => vec![],
    };

    let alias_fields = match platform {
      Platform::Browser => vec![vec!["browser".to_string()]],
      _ => vec![],
    };

    let builtin_modules = match platform {
      Platform::Node => true,
      Platform::Browser | Platform::Neutral => false,
    };

    let import_resolver = ResolverGeneric::new_with_cache(
      Arc::new(FsCache::new(fs)),
      OxcResolverOptions {
        main_fields,
        alias_fields,
        builtin_modules,
        condition_names: import_conditions,
        extensions: vec![String::from(".js"), String::from(".ts")],
        ..Default::default()
      },
    );

    Self { cwd, import_resolver }
  }
}

impl<F: FileSystem + Default> Resolver<F> {
  pub fn resolve(
    &self,
    importer: Option<&Path>,
    specifier: &str,
    is_user_defined_entry: bool,
  ) -> Result<ArcStr, ResolveError> {
    let resolver = &self.import_resolver;

    let dir = importer
      .and_then(|importer| importer.parent())
      .filter(|inner| inner.components().next().is_some())
      .unwrap_or(self.cwd.as_path());

    let mut resolution = resolver.resolve(dir, specifier);

    // Handle `{ input: 'main' }` -> `<CWD>/main.{js,mjs}`
    if resolution.is_err() && is_user_defined_entry && !specifier.starts_with(['.', '/']) {
      let normalized_specifier = self.cwd.join(specifier).normalize();
      let result = resolver.resolve(dir, &normalized_specifier.to_string_lossy());
      if result.is_ok() {
        resolution = result;
      }
    }

    resolution.map(|info| info.full_path().to_str().expect("Should be valid utf8").into())
  }
}
