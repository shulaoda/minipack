use std::path::{Path, PathBuf};

use arcstr::ArcStr;

use super::module_id::stabilize_module_id;

#[derive(Debug)]
pub struct ResolvedId {
  pub id: ArcStr,
  pub ignored: bool,
  pub is_external: bool,
  pub package_json: Option<PathBuf>,
}

impl ResolvedId {
  /// Created a pretty string representation of the path. The path
  /// 1. doesn't guarantee to be unique
  /// 2. relative to the cwd, so it could show stable path across different machines
  pub fn debug_id(&self, cwd: impl AsRef<Path>) -> String {
    if self.id.trim_start().starts_with("data:") {
      return format!("<{}>", self.id);
    }

    let stable = stabilize_module_id(&self.id, cwd.as_ref());

    if self.ignored { format!("(ignored) {stable}") } else { stable }
  }
}
