use std::{
  path::{Path, PathBuf},
  sync::Arc,
};

use arcstr::ArcStr;
use dashmap::DashMap;
use itertools::Itertools;
use sugar_path::SugarPath;

use oxc_resolver::{
  EnforceExtension, FsCache, PackageJsonSerde as OxcPackageJson, PackageType, Resolution,
  ResolveError, ResolveOptions as OxcResolverOptions, ResolverGeneric, TsConfigSerde,
  TsconfigOptions,
};

use minipack_common::{ImportKind, ModuleDefFormat, PackageJson, Platform, ResolveOptions};
use minipack_fs::{FileSystem, OsFileSystem};

#[derive(Debug)]
pub struct Resolver<T: FileSystem + Default = OsFileSystem> {
  cwd: PathBuf,
  default_resolver: ResolverGeneric<FsCache<T>>,
  // Resolver for `import '...'` and `import(...)`
  import_resolver: ResolverGeneric<FsCache<T>>,
  // Resolver for `require('...')`
  require_resolver: ResolverGeneric<FsCache<T>>,
  // Resolver for `@import '...'` and `url('...')`
  css_resolver: ResolverGeneric<FsCache<T>>,
  // Resolver for `new URL(..., import.meta.url)`
  new_url_resolver: ResolverGeneric<FsCache<T>>,
  package_json_cache: DashMap<PathBuf, Arc<PackageJson>>,
}

impl<F: FileSystem + Default> Resolver<F> {
  #[allow(clippy::too_many_lines)]
  pub fn new(raw_resolve: ResolveOptions, platform: Platform, cwd: PathBuf, fs: F) -> Self {
    let mut default_conditions = vec!["default".to_string()];
    let mut import_conditions = vec!["import".to_string()];
    let mut require_conditions = vec!["require".to_string()];

    default_conditions.extend(raw_resolve.condition_names.clone().unwrap_or_default());

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
    require_conditions.extend(default_conditions.clone());

    import_conditions = import_conditions.into_iter().unique().collect();
    require_conditions = require_conditions.into_iter().unique().collect();

    let main_fields = raw_resolve.main_fields.clone().unwrap_or_else(|| match platform {
      Platform::Node => {
        vec!["main".to_string(), "module".to_string()]
      }
      Platform::Browser => vec!["browser".to_string(), "module".to_string(), "main".to_string()],
      Platform::Neutral => vec![],
    });

    let alias_fields = raw_resolve.alias_fields.clone().unwrap_or_else(|| match platform {
      Platform::Browser => vec![vec!["browser".to_string()]],
      _ => vec![],
    });

    let builtin_modules = match platform {
      Platform::Node => true,
      Platform::Browser | Platform::Neutral => false,
    };

    let resolve_options_with_default_conditions = OxcResolverOptions {
      tsconfig: raw_resolve.tsconfig_filename.map(|p| {
        let path = PathBuf::from(&p);
        TsconfigOptions {
          config_file: if path.is_relative() { cwd.join(path) } else { path },
          references: oxc_resolver::TsconfigReferences::Disabled,
        }
      }),
      alias: raw_resolve
        .alias
        .map(|alias| {
          alias
            .into_iter()
            .map(|(key, value)| {
              (key, value.into_iter().map(oxc_resolver::AliasValue::Path).collect::<Vec<_>>())
            })
            .collect::<Vec<_>>()
        })
        .unwrap_or_default(),
      imports_fields: vec![vec!["imports".to_string()]],
      alias_fields,
      condition_names: default_conditions,
      description_files: vec!["package.json".to_string()],
      enforce_extension: EnforceExtension::Auto,
      exports_fields: raw_resolve
        .exports_fields
        .unwrap_or_else(|| vec![vec!["exports".to_string()]]),
      extension_alias: raw_resolve.extension_alias.unwrap_or_default(),
      extensions: raw_resolve.extensions.unwrap_or_else(|| {
        [".jsx", ".js", ".ts", ".tsx"].into_iter().map(str::to_string).collect()
      }),
      fallback: vec![],
      fully_specified: false,
      main_fields,
      main_files: raw_resolve.main_files.unwrap_or_else(|| vec!["index".to_string()]),
      modules: raw_resolve.modules.unwrap_or_else(|| vec!["node_modules".to_string()]),
      resolve_to_context: false,
      prefer_relative: false,
      prefer_absolute: false,
      restrictions: vec![],
      roots: vec![],
      symlinks: raw_resolve.symlinks.unwrap_or(true),
      builtin_modules,
    };

    let resolve_options_with_import_conditions = OxcResolverOptions {
      condition_names: import_conditions,
      ..resolve_options_with_default_conditions.clone()
    };

    let resolve_options_with_require_conditions = OxcResolverOptions {
      condition_names: require_conditions,
      ..resolve_options_with_default_conditions.clone()
    };

    let resolve_options_for_css = OxcResolverOptions {
      prefer_relative: true,
      ..resolve_options_with_default_conditions.clone()
    };

    let resolve_options_for_new_url = OxcResolverOptions {
      prefer_relative: true,
      ..resolve_options_with_default_conditions.clone()
    };

    let default_resolver = ResolverGeneric::new_with_cache(
      Arc::new(FsCache::new(fs)),
      resolve_options_with_default_conditions,
    );

    let import_resolver =
      default_resolver.clone_with_options(resolve_options_with_import_conditions);
    let require_resolver =
      default_resolver.clone_with_options(resolve_options_with_require_conditions);
    let css_resolver = default_resolver.clone_with_options(resolve_options_for_css);
    let new_url_resolver = default_resolver.clone_with_options(resolve_options_for_new_url);

    Self {
      cwd,
      default_resolver,
      import_resolver,
      require_resolver,
      css_resolver,
      new_url_resolver,
      package_json_cache: DashMap::default(),
    }
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
  pub module_def_format: ModuleDefFormat,
  pub package_json: Option<Arc<PackageJson>>,
}

fn infer_module_def_format<F: FileSystem + Default>(
  info: &Resolution<FsCache<F>>,
) -> ModuleDefFormat {
  let fmt = ModuleDefFormat::from_path(info.path());

  if !matches!(fmt, ModuleDefFormat::Unknown) {
    return fmt;
  }

  if let Some(package_json) = info.package_json() {
    return match package_json.r#type {
      Some(PackageType::CommonJs) => ModuleDefFormat::CjsPackageJson,
      Some(PackageType::Module) => ModuleDefFormat::EsmPackageJson,
      _ => ModuleDefFormat::Unknown,
    };
  }
  ModuleDefFormat::Unknown
}

impl<F: FileSystem + Default> Resolver<F> {
  pub fn resolve(
    &self,
    importer: Option<&Path>,
    specifier: &str,
    import_kind: ImportKind,
    is_user_defined_entry: bool,
  ) -> Result<ResolveReturn, ResolveError> {
    let resolver = match import_kind {
      ImportKind::NewUrl => &self.new_url_resolver,
      ImportKind::Require => &self.require_resolver,
      ImportKind::AtImport | ImportKind::UrlImport => &self.css_resolver,
      ImportKind::Import | ImportKind::DynamicImport => &self.import_resolver,
    };

    let dir = importer
      .and_then(|importer| importer.parent())
      .filter(|inner| inner.components().next().is_some())
      .unwrap_or(self.cwd.as_path());

    let mut resolution = resolver.resolve(dir, specifier);

    // Handle `{ input: 'main' }` -> `<CWD>/main.{js,mjs,cjs}`
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
      let path = info.full_path().to_str().expect("Should be valid utf8").to_string().into();
      let package_json = info.package_json().map(|p| self.cached_package_json(p));
      ResolveReturn { path, package_json, module_def_format: infer_module_def_format(&info) }
    })
  }

  fn cached_package_json(&self, oxc_pkg_json: &OxcPackageJson) -> Arc<PackageJson> {
    if let Some(v) = self.package_json_cache.get(&oxc_pkg_json.realpath) {
      Arc::clone(v.value())
    } else {
      let pkg_json = Arc::new(
        PackageJson::new(oxc_pkg_json.path.clone())
          .with_type(oxc_pkg_json.r#type.map(|t| match t {
            PackageType::CommonJs => "commonjs",
            PackageType::Module => "module",
          }))
          .with_side_effects(oxc_pkg_json.side_effects.as_ref()),
      );
      self.package_json_cache.insert(oxc_pkg_json.realpath.clone(), Arc::clone(&pkg_json));
      pkg_json
    }
  }
}
