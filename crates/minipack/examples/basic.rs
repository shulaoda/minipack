use std::path::PathBuf;
use sugar_path::SugarPath;

use minipack::{Bundler, BundlerOptions};

fn crate_dir(crate_name: &str) -> PathBuf {
  let root = PathBuf::from(env!("WORKSPACE_DIR"));
  root.join("crates").join(crate_name)
}

#[tokio::main]
async fn main() {
  let mut _bundler = Bundler::new(BundlerOptions {
    input: Some(vec!["./entry.js".to_string().into()]),
    cwd: Some(crate_dir("minipack").join("./examples/basic").normalize()),
    ..Default::default()
  });

  // let _result = bundler.write().await.unwrap();
}
