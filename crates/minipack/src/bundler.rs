use std::sync::Arc;

use crate::{
  generate_stage::GenerateStage,
  link_stage::LinkStage,
  scan_stage::ScanStage,
  types::{bundle_output::BundleOutput, SharedOptions, SharedResolver},
  utils::normalize_options::{normalize_options, NormalizeOptionsReturn},
};

use minipack_common::{BundlerOptions, NormalizedBundlerOptions};
use minipack_error::BuildResult;
use minipack_fs::{FileSystem, OsFileSystem};
use minipack_resolver::{ResolveError, Resolver};

pub struct Bundler {
  pub closed: bool,
  pub(crate) fs: OsFileSystem,
  pub(crate) options: SharedOptions,
  pub(crate) resolver: SharedResolver,
}

impl Bundler {
  pub fn new(options: BundlerOptions) -> Self {
    let NormalizeOptionsReturn { mut options, resolve_options } = normalize_options(options);

    let tsconfig_filename = resolve_options.tsconfig_filename.clone();

    let resolver: SharedResolver =
      Resolver::new(resolve_options, options.platform, options.cwd.clone(), OsFileSystem).into();

    Self::merge_transform_config_from_ts_config(&mut options, tsconfig_filename, &resolver)
      .unwrap();

    Bundler { closed: false, fs: OsFileSystem, options: Arc::new(options), resolver }
  }

  pub async fn write(&mut self) -> BuildResult<BundleOutput> {
    self.build(true).await
  }

  pub async fn generate(&mut self) -> BuildResult<BundleOutput> {
    self.build(false).await
  }

  async fn build(&mut self, is_write: bool) -> BuildResult<BundleOutput> {
    if self.closed {
      Err(anyhow::anyhow!(
        "Bundle is already closed, no more calls to 'generate' or 'write' are allowed."
      ))?;
    }

    let scan_stage_output =
      ScanStage::new(self.fs, self.options.clone(), self.resolver.clone()).scan().await?;

    let mut link_stage_output = LinkStage::new(scan_stage_output, &self.options).link();

    let bundle_output =
      GenerateStage::new(&mut link_stage_output, &self.options).generate().await?;

    if is_write {
      let dist = self.options.cwd.join(&self.options.dir);

      self.fs.create_dir_all(&dist).map_err(|err| {
        anyhow::anyhow!("Could not create directory for output chunks: {:?}", dist).context(err)
      })?;

      for chunk in &bundle_output.assets {
        let dest = dist.join(chunk.filename());
        if let Some(p) = dest.parent() {
          if !self.fs.exists(p) {
            self.fs.create_dir_all(p).unwrap();
          }
        };
        self
          .fs
          .write(&dest, chunk.content_as_bytes())
          .map_err(|err| anyhow::anyhow!("Failed to write file in {:?}", dest).context(err))?;
      }
    }

    Ok(bundle_output)
  }

  fn merge_transform_config_from_ts_config(
    options: &mut NormalizedBundlerOptions,
    tsconfig_filename: Option<String>,
    resolver: &SharedResolver,
  ) -> Result<(), ResolveError> {
    let Some(tsconfig_filename) = tsconfig_filename else {
      return Ok(());
    };
    let ts_config = resolver.resolve_tsconfig(&tsconfig_filename)?;
    if let Some(ref jsx_factory) = ts_config.compiler_options.jsx_factory {
      options.base_transform_options.jsx.pragma = Some(jsx_factory.clone());
    }
    if let Some(ref jsx_fragment_factory) = ts_config.compiler_options.jsx_fragment_factory {
      options.base_transform_options.jsx.pragma_frag = Some(jsx_fragment_factory.clone());
    }
    if let Some(ref jsx_import_source) = ts_config.compiler_options.jsx_import_source {
      options.base_transform_options.jsx.import_source = Some(jsx_import_source.clone());
    }
    if let Some(ref experimental_decorator) = ts_config.compiler_options.experimental_decorators {
      options.base_transform_options.decorator.legacy = *experimental_decorator;
    }
    Ok(())
  }
}

#[test]
fn test_rust_syntax_errors() {}
