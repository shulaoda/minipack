use minipack_common::NormalizedBundlerOptions;
use minipack_error::BuildResult;

pub fn validate_options_for_multi_chunk_output(
  options: &NormalizedBundlerOptions,
) -> BuildResult<()> {
  options.file.as_ref().map_or(Ok(()), |_| {
    Err(anyhow::anyhow!("Invalid value for option \"output.file\" - When building multiple chunks, the \"output.dir\" option must be used, not \"output.file\". You may set `output.inlineDynamicImports` to `true` when using dynamic imports."))?
  })
}
