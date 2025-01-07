use crate::types::generator::GenerateContext;
use arcstr::ArcStr;
use minipack_common::{NormalModule, OutputExports};
use minipack_error::BuildResult;
use minipack_utils::rstr::Rstr;

// Port from https://github.com/rollup/rollup/blob/master/src/utils/getExportMode.ts
pub fn determine_export_mode(
  warnings: &mut Vec<anyhow::Error>,
  ctx: &GenerateContext<'_>,
  module: &NormalModule,
  export_names: &[Rstr],
) -> BuildResult<OutputExports> {
  let export_mode = &ctx.options.exports;
  match export_mode {
    OutputExports::Named => Ok(OutputExports::Named),
    OutputExports::Default => {
      if export_names.len() != 1 || export_names[0].as_str() != "default" {
        Err(anyhow::anyhow!(
          r#""{}" was specified for "output.exports", but entry module "{}" has the following exports: {}."#,
          "default",
          module.stable_id.as_str(),
          export_names.iter().map(|k| format!(r#""{k}""#)).collect::<Vec<_>>().join(", ")
        ))?;
      }
      Ok(OutputExports::Default)
    }
    OutputExports::None => {
      if !export_names.is_empty() {
        Err(anyhow::anyhow!(
          r#""{}" was specified for "output.exports", but entry module "{}" has the following exports: {}."#,
          "none",
          module.stable_id.as_str(),
          export_names.iter().map(|k| format!(r#""{k}""#)).collect::<Vec<_>>().join(", ")
        ))?;
      }
      Ok(OutputExports::None)
    }
    OutputExports::Auto => {
      if export_names.is_empty() {
        Ok(OutputExports::None)
      } else if export_names.len() == 1 && export_names[0].as_str() == "default" {
        Ok(OutputExports::Default)
      } else {
        let has_default_export = export_names.iter().any(|name| name.as_str() == "default");
        if has_default_export {
          let name = &ctx.chunk.name;
          let chunk = ArcStr::from("chunk");
          let name = name.as_ref().unwrap_or(&chunk);

          warnings.push(anyhow::anyhow!(r#"Entry module "{}" is using named (including {}) and default exports together. Consumers of your bundle will have to use `{}.default` to access the default export, which may not be what you want. Use `output.exports: "named"` to disable this warning."#,
            module.stable_id.as_str(),
          export_names.iter().map(|k| format!(r#""{k}""#)).collect::<Vec<_>>().join(", "),
          name));
        }
        Ok(OutputExports::Named)
      }
    }
  }
}
