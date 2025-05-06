use std::path::Path;

use minipack_common::ResolvedId;
use minipack_error::BuildResult;
use minipack_resolver::{ResolveError, Resolver};

pub fn resolve_id(
  resolver: &Resolver,
  request: &str,
  importer: Option<&str>,
  is_user_defined_entry: bool,
) -> BuildResult<ResolvedId> {
  match resolver.resolve(importer.map(Path::new), request, is_user_defined_entry) {
    Ok(resolved) => Ok(ResolvedId {
      id: resolved.path,
      is_external: false,
      package_json: resolved.package_json,
    }),
    Err(err) => match err {
      ResolveError::Builtin { resolved, is_runtime_module } => Ok(ResolvedId {
        id: if resolved.starts_with("node:") && !is_runtime_module {
          resolved[5..].into()
        } else {
          resolved.into()
        },
        is_external: true,
        package_json: None,
      }),
      _ => Err(anyhow::anyhow!("{:?}", err))?,
    },
  }
}
