use std::path::PathBuf;
use sugar_path::SugarPath;

use minipack::{Bundler, BundlerOptions};

#[tokio::main]
async fn main() {
  let root = PathBuf::from(env!("WORKSPACE_DIR"));
  let root = root.join("crates/minipack/examples/basic");

  let mut bundler = Bundler::new(BundlerOptions {
    input: Some(vec!["./entry.js".to_string().into()]),
    cwd: Some(root.normalize()),
    ..Default::default()
  });

  let _ = bundler.write().await;
}
