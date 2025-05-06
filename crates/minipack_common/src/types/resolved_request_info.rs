use std::path::{Path, PathBuf};

use arcstr::ArcStr;

use super::module_id::stabilize_module_id;

#[derive(Debug)]
pub struct ResolvedId {
  pub id: ArcStr,
  pub is_external: bool,
  pub package_json: Option<PathBuf>,
}

impl ResolvedId {
  pub fn debug_id(&self, cwd: impl AsRef<Path>) -> String {
    if self.id.trim_start().starts_with("data:") {
      return format!("<{}>", self.id);
    }
    stabilize_module_id(&self.id, cwd.as_ref())
  }
}
