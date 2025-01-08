use std::path::Path;

use minipack_common::{is_existing_node_builtin_modules, ImportKind, ModuleDefFormat, ResolvedId};
use minipack_error::BuildResult;
use minipack_resolver::{ResolveError, Resolver};

#[inline]
fn is_http_url(s: &str) -> bool {
  s.starts_with("http://") || s.starts_with("https://") || s.starts_with("//")
}

#[inline]
fn is_data_url(s: &str) -> bool {
  s.trim_start().starts_with("data:")
}

pub fn resolve_id(
  resolver: &Resolver,
  request: &str,
  importer: Option<&str>,
  import_kind: ImportKind,
  is_user_defined_entry: bool,
) -> BuildResult<ResolvedId> {
  // Auto external http url or data url
  if is_http_url(request) || is_data_url(request) {
    return Ok(ResolvedId {
      id: request.to_string().into(),
      module_def_format: ModuleDefFormat::Unknown,
      ignored: false,
      is_external: true,
      package_json: None,
      side_effects: None,
      is_external_without_side_effects: false,
    });
  }

  let resolved =
    resolver.resolve(importer.map(Path::new), request, import_kind, is_user_defined_entry);

  match resolved {
    Ok(resolved) => Ok(ResolvedId {
      id: resolved.path,
      ignored: false,
      module_def_format: resolved.module_def_format,
      is_external: false,
      package_json: resolved.package_json,
      side_effects: None,
      is_external_without_side_effects: false,
    }),
    Err(err) => match err {
      ResolveError::Builtin { resolved, is_runtime_module } => Ok(ResolvedId {
        // `resolved` is always prefixed with "node:" in compliance with the ESM specification.
        // we needs to use `is_runtime_module` to get the original specifier
        is_external_without_side_effects: is_existing_node_builtin_modules(&resolved),
        id: if resolved.starts_with("node:") && !is_runtime_module {
          resolved[5..].to_string().into()
        } else {
          resolved.into()
        },
        ignored: false,
        is_external: true,
        module_def_format: ModuleDefFormat::Unknown,
        package_json: None,
        side_effects: None,
      }),
      ResolveError::Ignored(p) => Ok(ResolvedId {
        id: p.to_str().expect("Should be valid utf8").into(),
        ignored: true,
        is_external: false,
        module_def_format: ModuleDefFormat::Unknown,
        package_json: None,
        side_effects: None,
        is_external_without_side_effects: false,
      }),
      _ => Err(anyhow::anyhow!("{:?}", err))?,
    },
  }
}
