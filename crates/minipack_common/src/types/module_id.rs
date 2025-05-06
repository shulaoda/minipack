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
    stabilize_module_id(&self.0, cwd)
  }
}

impl AsRef<str> for ModuleId {
  fn as_ref(&self) -> &str {
    &self.0
  }
}

impl std::ops::Deref for ModuleId {
  type Target = str;

  fn deref(&self) -> &Self::Target {
    &self.0
  }
}

impl From<ArcStr> for ModuleId {
  fn from(value: ArcStr) -> Self {
    Self::new(value)
  }
}

pub fn stabilize_module_id(module_id: &str, cwd: &Path) -> String {
  if module_id.as_path().is_absolute() {
    module_id.relative(cwd).as_path().to_slash_lossy().into_owned()
  } else {
    module_id.to_string()
  }
}

#[test]
fn test_stabilize_module_id() {
  let cwd = std::env::current_dir().unwrap();
  // absolute path
  assert_eq!(
    stabilize_module_id(&cwd.join("src").join("main.js").to_string_lossy(), &cwd),
    "src/main.js"
  );
  assert_eq!(
    stabilize_module_id(&cwd.join("..").join("src").join("main.js").to_string_lossy(), &cwd),
    "../src/main.js"
  );

  // non-path specifier
  assert_eq!(stabilize_module_id("fs", &cwd), "fs");
  assert_eq!(
    stabilize_module_id("https://deno.land/x/oak/mod.ts", &cwd),
    "https://deno.land/x/oak/mod.ts"
  );
}
