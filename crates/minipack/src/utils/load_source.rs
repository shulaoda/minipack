use std::path::Path;

use minipack_common::{ModuleType, ResolvedId, StrOrBytes};
use minipack_fs::FileSystem;
use rustc_hash::FxHashMap;
use sugar_path::SugarPath;

pub fn load_source(
  fs: &dyn FileSystem,
  resolved_id: &ResolvedId,
) -> anyhow::Result<(StrOrBytes, ModuleType)> {
  if resolved_id.ignored {
    Ok((String::new().into(), ModuleType::Empty))
  } else {
    if let Some(guessed) = get_module_loader_from_file_extension(&resolved_id.id) {
      match &guessed {
        ModuleType::Base64 | ModuleType::Binary | ModuleType::Dataurl | ModuleType::Asset => {
          Ok((StrOrBytes::Bytes(fs.read(resolved_id.id.as_path())?), guessed))
        }
        ModuleType::Js
        | ModuleType::Jsx
        | ModuleType::Ts
        | ModuleType::Tsx
        | ModuleType::Json
        | ModuleType::Text
        | ModuleType::Empty
        | ModuleType::Css
        | ModuleType::Custom(_) => {
          Ok((StrOrBytes::Str(fs.read_to_string(resolved_id.id.as_path())?), guessed))
        }
      }
    } else {
      Ok((StrOrBytes::Str(fs.read_to_string(resolved_id.id.as_path())?), ModuleType::Js))
    }
  }
}

/// ref: https://github.com/evanw/esbuild/blob/9c13ae1f06dfa909eb4a53882e3b7e4216a503fe/internal/bundler/bundler.go#L1161-L1183
fn get_module_loader_from_file_extension<S: AsRef<str>>(id: S) -> Option<ModuleType> {
  let id = id.as_ref();
  let loaders = FxHashMap::from(
    [
      ("js".to_string(), ModuleType::Js),
      ("mjs".to_string(), ModuleType::Js),
      ("cjs".to_string(), ModuleType::Js),
      ("jsx".to_string(), ModuleType::Jsx),
      ("ts".to_string(), ModuleType::Ts),
      ("mts".to_string(), ModuleType::Ts),
      ("cts".to_string(), ModuleType::Ts),
      ("tsx".to_string(), ModuleType::Tsx),
      ("json".to_string(), ModuleType::Json),
      ("txt".to_string(), ModuleType::Text),
      ("css".to_string(), ModuleType::Css),
    ]
    .into_iter()
    .collect(),
  );

  if let Some(ext) = id.rsplit('.').next().filter(|ext| *ext != id) {
    if let Some(ty) = loaders.get(ext) {
      return Some(ty.clone());
    }
  };

  None
}

fn read_file_by_module_type(
  path: impl AsRef<Path>,
  ty: &ModuleType,
  fs: &dyn FileSystem,
) -> anyhow::Result<StrOrBytes> {
  let path = path.as_ref();
  match ty {
    ModuleType::Js
    | ModuleType::Jsx
    | ModuleType::Ts
    | ModuleType::Tsx
    | ModuleType::Json
    | ModuleType::Css
    | ModuleType::Empty
    | ModuleType::Custom(_)
    | ModuleType::Text => Ok(StrOrBytes::Str(fs.read_to_string(path)?)),
    ModuleType::Base64 | ModuleType::Binary | ModuleType::Dataurl | ModuleType::Asset => {
      Ok(StrOrBytes::Bytes(fs.read(path)?))
    }
  }
}
