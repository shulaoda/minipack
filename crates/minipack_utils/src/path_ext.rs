use std::{borrow::Cow, ffi::OsStr};

use sugar_path::SugarPath;

pub trait PathExt {
  fn expect_to_str(&self) -> &str;

  fn expect_to_slash(&self) -> String;

  fn representative_file_name(&self) -> Cow<str>;
}

impl PathExt for std::path::Path {
  fn expect_to_str(&self) -> &str {
    self.to_str().unwrap_or_else(|| {
      panic!("Failed to convert {:?} to valid utf8 str", self.display());
    })
  }

  fn expect_to_slash(&self) -> String {
    self
      .to_slash()
      .unwrap_or_else(|| panic!("Failed to convert {:?} to slash str", self.display()))
      .into_owned()
  }

  /// It doesn't ensure the file name is a valid identifier in JS.
  fn representative_file_name(&self) -> Cow<str> {
    let file_name =
      self.file_stem().map_or_else(|| self.to_string_lossy(), |stem| stem.to_string_lossy());

    let file_name = match &*file_name {
      // "index": Node.js use `index` as a special name for directory import.
      // "mod": https://docs.deno.com/runtime/manual/references/contributing/style_guide#do-not-use-the-filename-indextsindexjs.
      "index" | "mod" => self
        .parent()
        .and_then(Self::file_stem)
        .map(OsStr::to_string_lossy)
        .map_or(file_name, |parent_dir_name| parent_dir_name),
      _ => file_name,
    };

    file_name
  }
}

#[test]
fn test_representative_file_name() {
  use std::path::Path;

  let cwd = Path::new(".").join("project");
  let path = cwd.join("src").join("vue.js");
  assert_eq!(path.representative_file_name(), "vue");

  let path = cwd.join("vue").join("index.js");
  assert_eq!(path.representative_file_name(), "vue");

  let path = cwd.join("vue").join("mod.ts");
  assert_eq!(path.representative_file_name(), "vue");
}
