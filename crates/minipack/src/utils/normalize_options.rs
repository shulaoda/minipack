use std::sync::Arc;

use minipack_common::{BundlerOptions, NormalizedBundlerOptions, OutputFormat, Platform};

pub fn normalize_options(raw_options: BundlerOptions) -> Arc<NormalizedBundlerOptions> {
  let cwd =
    raw_options.cwd.unwrap_or_else(|| std::env::current_dir().expect("Failed to get current dir"));

  let dir = raw_options.dir.unwrap_or_else(|| "dist".to_string());
  let format = raw_options.format.unwrap_or_default();
  let platform = raw_options.platform.unwrap_or(match format {
    OutputFormat::Cjs => Platform::Node,
    OutputFormat::Esm => Platform::Browser,
  });

  Arc::new(NormalizedBundlerOptions {
    // --- Input
    cwd,
    input: raw_options.input.unwrap_or_default(),
    platform,
    // --- Output
    dir,
    format,
    entry_filenames: raw_options.entry_filenames.unwrap_or_else(|| "[name].js".to_string()),
    chunk_filenames: raw_options.chunk_filenames.unwrap_or_else(|| "[name]-[hash].js".to_string()),
    // --- Enhance
    minify: raw_options.minify.unwrap_or_default(),
  })
}
