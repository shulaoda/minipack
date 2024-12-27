use minipack_common::{NormalizedBundlerOptions, OutputFormat, Platform};

pub struct NormalizeOptionsReturn {
  pub options: NormalizedBundlerOptions,
  pub resolve_options: minipack_resolver::ResolveOptions,
}

pub fn normalize_options(mut raw_options: crate::BundlerOptions) -> NormalizeOptionsReturn {
  let format = raw_options.format.unwrap_or(crate::OutputFormat::Esm);
  let platform = raw_options.platform.unwrap_or(match format {
    OutputFormat::Cjs => Platform::Node,
    OutputFormat::Esm => Platform::Browser,
  });

  let raw_resolve = std::mem::take(&mut raw_options.resolve).unwrap_or_default();

  let normalized = NormalizedBundlerOptions {
    input: raw_options.input.unwrap_or_default(),
    cwd: raw_options
      .cwd
      .unwrap_or_else(|| std::env::current_dir().expect("Failed to get current dir")),
    platform,
    shim_missing_exports: raw_options.shim_missing_exports.unwrap_or(false),
    name: raw_options.name,
    entry_filenames: raw_options.entry_filenames.unwrap_or_else(|| "[name].js".to_string()),
    chunk_filenames: raw_options.chunk_filenames.unwrap_or_else(|| "[name]-[hash].js".to_string()),
    asset_filenames: raw_options
      .asset_filenames
      .unwrap_or_else(|| "assets/[name]-[hash][extname]".to_string()),
    css_entry_filenames: raw_options
      .css_entry_filenames
      .unwrap_or_else(|| "[name].css".to_string().into()),
    css_chunk_filenames: raw_options
      .css_chunk_filenames
      .unwrap_or_else(|| "[name]-[hash].css".to_string().into()),
    dir: raw_options.dir.unwrap_or_else(|| "dist".to_string()),
    file: raw_options.file,
    format,
    exports: raw_options.exports.unwrap_or(crate::OutputExports::Auto),
    es_module: raw_options.es_module.unwrap_or_default(),
    target: raw_options.target.unwrap_or_default(),
    inline_dynamic_imports: raw_options.inline_dynamic_imports.unwrap_or_default(),
  };

  NormalizeOptionsReturn { options: normalized, resolve_options: raw_resolve }
}
