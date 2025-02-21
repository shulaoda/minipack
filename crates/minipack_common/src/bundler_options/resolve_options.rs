#[derive(Debug, Default, Clone)]
pub struct ResolveOptions {
  pub alias: Option<Vec<(String, Vec<String>)>>,
  pub alias_fields: Option<Vec<Vec<String>>>,
  pub condition_names: Option<Vec<String>>,
  pub exports_fields: Option<Vec<Vec<String>>>,
  pub extensions: Option<Vec<String>>,
  pub extension_alias: Option<Vec<(String, Vec<String>)>>,
  pub main_fields: Option<Vec<String>>,
  pub main_files: Option<Vec<String>>,
  pub modules: Option<Vec<String>>,
  pub symlinks: Option<bool>,
  pub tsconfig_filename: Option<String>,
}
