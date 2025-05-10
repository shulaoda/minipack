use std::{
  path::{Path, PathBuf},
  sync::Arc,
};

use minipack_common::{Platform, ResolvedId};
use minipack_fs::{FileSystem, OsFileSystem};
use sugar_path::SugarPath as _;

use oxc_resolver::{FsCache, ResolveError, ResolveOptions as OxcResolverOptions, ResolverGeneric};

#[derive(Debug)]
pub struct Resolver<T: FileSystem + Default = OsFileSystem> {
  cwd: PathBuf,
  import_resolver: ResolverGeneric<FsCache<T>>,
}

impl<F: FileSystem + Default> Resolver<F> {
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
  pub fn resolve_id(
    &self,
    request: &str,
    importer: Option<&str>,
    is_user_defined_entry: bool,
  ) -> minipack_error::BuildResult<ResolvedId> {
    let importer = importer.map(Path::new);
    let resolver = &self.import_resolver;

    let dir = importer
      .and_then(|importer| importer.parent())
      .filter(|inner| inner.components().next().is_some())
      .unwrap_or(self.cwd.as_path());

    let mut resolution = resolver.resolve(dir, request);

    // Handle `{ input: 'main' }` -> `<CWD>/main.{js,mjs}`
    if resolution.is_err() && is_user_defined_entry && !request.starts_with(['.', '/']) {
      let specifier = self.cwd.join(request).normalize();
      let result = resolver.resolve(dir, &specifier.to_string_lossy());
      if result.is_ok() {
        resolution = result;
      }
    }

    match resolution.map(|info| info.full_path().to_string_lossy().into()) {
      Ok(id) => Ok(ResolvedId { id, is_external: false }),
      Err(err) => match err {
        ResolveError::Builtin { resolved, is_runtime_module } => Ok(ResolvedId {
          id: if resolved.starts_with("node:") && !is_runtime_module {
            resolved[5..].into()
          } else {
            resolved.into()
          },
          is_external: true,
        }),
        _ => Err(anyhow::anyhow!("{:?}", err))?,
      },
    }
  }
}
