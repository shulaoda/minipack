use std::{borrow::Cow, ffi::OsStr};

pub trait PathExt {
  fn representative_file_name(&self) -> Cow<str>;
}

impl PathExt for std::path::Path {
  /// It doesn't ensure the file name is a valid identifier in JS.
  fn representative_file_name(&self) -> Cow<str> {
    let file_name =
      self.file_stem().map_or_else(|| self.to_string_lossy(), |s| s.to_string_lossy());
    match &*file_name {
      // "index": Node.js use `index` as a special name for directory import.
      // "mod": https://docs.deno.com/runtime/manual/references/contributing/style_guide#do-not-use-the-filename-indextsindexjs.
      "index" | "mod" => self
        .parent()
        .and_then(Self::file_stem)
        .map(OsStr::to_string_lossy)
        .map_or(file_name, |parent_dir_name| parent_dir_name),
      _ => file_name,
    }
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
