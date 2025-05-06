use std::{
  path::{Path, PathBuf},
  sync::Arc,
};

use arcstr::ArcStr;
use dashmap::DashMap;
use itertools::Itertools;
use sugar_path::SugarPath;

use oxc_resolver::{
  FsCache, PackageJsonSerde as OxcPackageJson, ResolveError, ResolveOptions as OxcResolverOptions,
  ResolverGeneric, TsConfigSerde,
};

use minipack_common::Platform;
use minipack_fs::{FileSystem, OsFileSystem};

#[derive(Debug)]
pub struct Resolver<T: FileSystem + Default = OsFileSystem> {
  cwd: PathBuf,
  import_resolver: ResolverGeneric<FsCache<T>>,
  default_resolver: ResolverGeneric<FsCache<T>>,
  package_json_cache: DashMap<PathBuf, PathBuf>,
}

impl<F: FileSystem + Default> Resolver<F> {
  #[allow(clippy::too_many_lines)]
  pub fn new(platform: Platform, cwd: PathBuf, fs: F) -> Self {
    let mut default_conditions = vec!["default".to_string()];
    let mut import_conditions = vec!["import".to_string()];

    match platform {
      Platform::Node => {
        default_conditions.push("node".to_string());
      }
      Platform::Browser => {
        default_conditions.push("browser".to_string());
      }
      Platform::Neutral => {}
    }

    default_conditions = default_conditions.into_iter().unique().collect();

    import_conditions.extend(default_conditions.clone());

    import_conditions = import_conditions.into_iter().unique().collect();

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

    let resolve_options_with_default_conditions = OxcResolverOptions {
      main_fields,
      alias_fields,
      builtin_modules,
      condition_names: default_conditions,
      extensions: vec![String::from(".js"), String::from(".ts")],
      ..Default::default()
    };

    let resolve_options_with_import_conditions = OxcResolverOptions {
      condition_names: import_conditions,
      ..resolve_options_with_default_conditions.clone()
    };

    let default_resolver = ResolverGeneric::new_with_cache(
      Arc::new(FsCache::new(fs)),
      resolve_options_with_default_conditions,
    );

    let import_resolver =
      default_resolver.clone_with_options(resolve_options_with_import_conditions);

    Self { cwd, default_resolver, import_resolver, package_json_cache: DashMap::default() }
  }

  pub fn cwd(&self) -> &PathBuf {
    &self.cwd
  }

  pub fn resolve_tsconfig<T: AsRef<Path>>(
    &self,
    path: &T,
  ) -> Result<Arc<TsConfigSerde>, ResolveError> {
    self.default_resolver.resolve_tsconfig(path)
  }
}

#[derive(Debug)]
pub struct ResolveReturn {
  pub path: ArcStr,
  pub package_json: Option<PathBuf>,
}

impl<F: FileSystem + Default> Resolver<F> {
  pub fn resolve(
    &self,
    importer: Option<&Path>,
    specifier: &str,
    is_user_defined_entry: bool,
  ) -> Result<ResolveReturn, ResolveError> {
    let resolver = &self.import_resolver;

    let dir = importer
      .and_then(|importer| importer.parent())
      .filter(|inner| inner.components().next().is_some())
      .unwrap_or(self.cwd.as_path());

    let mut resolution = resolver.resolve(dir, specifier);

    // Handle `{ input: 'main' }` -> `<CWD>/main.{js,mjs}`
    if resolution.is_err() && is_user_defined_entry {
      let is_specifier_path_like = specifier.starts_with('.') || specifier.starts_with('/');
      let need_rollup_resolve_compat = !is_specifier_path_like;

      if need_rollup_resolve_compat {
        let normalized_specifier = self.cwd.join(specifier).normalize();
        let result = resolver.resolve(dir, &normalized_specifier.to_string_lossy());
        if result.is_ok() {
          resolution = result;
        }
      }
    }

    resolution.map(|info| {
      let path = info.full_path().to_str().expect("Should be valid utf8").into();
      let package_json = info.package_json().map(|p| self.cached_package_json(p));
      ResolveReturn { path, package_json }
    })
  }

  fn cached_package_json(&self, oxc_pkg_json: &OxcPackageJson) -> PathBuf {
    self.package_json_cache.get(&oxc_pkg_json.realpath).map_or_else(
      || {
        self.package_json_cache.insert(oxc_pkg_json.realpath.clone(), oxc_pkg_json.path.clone());
        oxc_pkg_json.path.clone()
      },
      |v| v.value().clone(),
    )
  }
}
