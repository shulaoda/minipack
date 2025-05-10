use std::path::Path;

use arcstr::ArcStr;
use sugar_path::SugarPath;

/// `ModuleId` is the unique string identifier for each module.
/// - It will be used to identify the module in the whole bundle.
/// - Users could stored the `ModuleId` to track the module in different stages/hooks.
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Clone)]
pub struct ModuleId(ArcStr);

impl ModuleId {
  pub fn new(value: impl Into<ArcStr>) -> Self {
    Self(value.into())
  }

  pub fn stabilize(&self, cwd: &Path) -> String {
    if self.as_path().is_absolute() {
      self.relative(cwd).as_path().to_slash_lossy().into_owned()
    } else {
      self.to_string()
    }
  }
}

impl std::ops::Deref for ModuleId {
  type Target = str;

  fn deref(&self) -> &Self::Target {
    &self.0
  }
}

impl AsRef<str> for ModuleId {
  fn as_ref(&self) -> &str {
    self
  }
}

impl From<ArcStr> for ModuleId {
  fn from(value: ArcStr) -> Self {
    Self::new(value)
  }
}
