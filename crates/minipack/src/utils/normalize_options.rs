use std::path::Path;

use minipack_common::{
  BundlerOptions, NormalizedBundlerOptions, OutputExports, OutputFormat, Platform, ResolveOptions,
};
use oxc::transformer::{ESTarget, TransformOptions};

pub struct NormalizeOptionsReturn {
  pub options: NormalizedBundlerOptions,
  pub resolve_options: ResolveOptions,
}

pub fn normalize_options(mut raw_options: BundlerOptions) -> NormalizeOptionsReturn {
  let cwd =
    raw_options.cwd.unwrap_or_else(|| std::env::current_dir().expect("Failed to get current dir"));

  let format = raw_options.format.unwrap_or(OutputFormat::Esm);
  let platform = raw_options.platform.unwrap_or(match format {
    OutputFormat::Cjs => Platform::Node,
    OutputFormat::Esm => Platform::Browser,
  });

  let resolve_options = std::mem::take(&mut raw_options.resolve).unwrap_or_default();

  let dir = raw_options.file.as_ref().map_or(
    raw_options.dir.unwrap_or_else(|| "dist".to_string()),
    |file| {
      Path::new(file.as_str())
        .parent()
        .map(|parent| parent.to_string_lossy().to_string())
        .unwrap_or_default()
    },
  );

  let target = raw_options.target.unwrap_or_default();
  let base_transform_options = TransformOptions::from(ESTarget::from(target));

  let options = NormalizedBundlerOptions {
    // --- Input
    cwd,
    input: raw_options.input.unwrap_or_default(),
    platform,
    // --- Output
    dir,
    file: raw_options.file,
    format,
    exports: raw_options.exports.unwrap_or(OutputExports::Auto),
    entry_filenames: raw_options.entry_filenames.unwrap_or_else(|| "[name].js".to_string()),
    chunk_filenames: raw_options.chunk_filenames.unwrap_or_else(|| "[name]-[hash].js".to_string()),
    asset_filenames: raw_options
      .asset_filenames
      .unwrap_or_else(|| "assets/[name]-[hash][extname]".to_string()),
    css_entry_filenames: raw_options
      .css_entry_filenames
      .unwrap_or_else(|| "[name].css".to_string()),
    css_chunk_filenames: raw_options
      .css_chunk_filenames
      .unwrap_or_else(|| "[name]-[hash].css".to_string()),
    // --- Enhance
    minify: raw_options.minify.unwrap_or_default(),
    target,
    shim_missing_exports: raw_options.shim_missing_exports.unwrap_or_default(),
    inline_dynamic_imports: raw_options.inline_dynamic_imports.unwrap_or_default(),
    base_transform_options,
  };

  NormalizeOptionsReturn { options, resolve_options }
}
