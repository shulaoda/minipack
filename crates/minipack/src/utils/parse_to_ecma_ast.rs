use std::path::Path;

use arcstr::ArcStr;
use minipack_common::{
  ModuleType, NormalizedBundlerOptions, Platform, RUNTIME_MODULE_ID, StrOrBytes,
};
use minipack_ecmascript::{EcmaAst, EcmaCompiler};
use minipack_error::BuildResult;
use minipack_utils::{
  base64::to_standard_base64, concat_string, mime::guess_mime,
  percent_encoding::encode_as_percent_escaped, text_to_esm::text_to_string_literal,
};
use oxc::{semantic::Scoping, span::SourceType as OxcSourceType};
use sugar_path::SugarPath;

use super::pre_process_ecma_ast::PreProcessEcmaAst;

use crate::types::{module_factory::CreateModuleContext, oxc_parse_type::OxcParseType};

fn binary_to_esm(base64: &str, platform: Platform, runtime_module_id: &str) -> String {
  let to_binary = match platform {
    Platform::Node => "__toBinaryNode",
    _ => "__toBinary",
  };
  [
    "import {",
    to_binary,
    "} from '",
    runtime_module_id,
    "'; export default ",
    to_binary,
    "('",
    base64,
    "')",
  ]
  .concat()
}

pub struct ParseToEcmaAstResult {
  pub ast: EcmaAst,
  pub scoping: Scoping,
  pub has_lazy_export: bool,
  pub warning: Vec<anyhow::Error>,
}

pub fn parse_to_ecma_ast(
  ctx: &CreateModuleContext<'_>,
  source: StrOrBytes,
) -> BuildResult<ParseToEcmaAstResult> {
  let CreateModuleContext { options, stable_id, resolved_id, module_type, .. } = ctx;

  let path = resolved_id.id.as_path();
  let is_user_defined_entry = ctx.is_user_defined_entry;

  let (has_lazy_export, source, parsed_type) =
    pre_process_source(module_type, source, is_user_defined_entry, path, options)?;

  let oxc_source_type = {
    let default = OxcSourceType::default().with_module(true);
    match parsed_type {
      OxcParseType::Js | OxcParseType::Jsx => default,
      OxcParseType::Ts | OxcParseType::Tsx => default.with_typescript(true),
    }
  };

  let ecma_ast = match module_type {
    ModuleType::Json | ModuleType::Dataurl | ModuleType::Base64 | ModuleType::Text => {
      EcmaCompiler::parse_expr_as_program(&source, oxc_source_type)?
    }
    _ => EcmaCompiler::parse(&source, oxc_source_type)?,
  };

  PreProcessEcmaAst::default().build(
    ecma_ast,
    stable_id.as_path(),
    &parsed_type,
    has_lazy_export,
    options,
  )
}

fn pre_process_source(
  module_type: &ModuleType,
  source: StrOrBytes,
  is_user_defined_entry: bool,
  path: &Path,
  options: &NormalizedBundlerOptions,
) -> BuildResult<(bool, ArcStr, OxcParseType)> {
  let mut has_lazy_export = matches!(
    module_type,
    ModuleType::Json
      | ModuleType::Text
      | ModuleType::Base64
      | ModuleType::Dataurl
      | ModuleType::Asset
  );

  let source = match module_type {
    ModuleType::Js | ModuleType::Jsx | ModuleType::Ts | ModuleType::Tsx | ModuleType::Json => {
      source.try_into_string()?
    }
    ModuleType::Css => {
      if is_user_defined_entry {
        "export {}".to_owned()
      } else {
        has_lazy_export = true;
        "({})".to_owned()
      }
    }
    ModuleType::Text => text_to_string_literal(&source.try_into_string()?)?,
    ModuleType::Base64 => text_to_string_literal(&to_standard_base64(source.into_bytes()))?,
    ModuleType::Dataurl => {
      let data = source.into_bytes();
      let guessed_mime = guess_mime(path, &data)?;

      let base64 = to_standard_base64(&data);
      let mime_ext_string = guessed_mime.to_string();
      let base64_url = concat_string!("data:", mime_ext_string, ";base64,", base64);

      let encoded_data = encode_as_percent_escaped(&data)
        .map(|encoded| concat_string!("data:", mime_ext_string, ",", encoded));

      let dataurl = match encoded_data {
        Some(percent_url) if percent_url.len() < base64_url.len() => percent_url,
        _ => base64_url,
      };

      text_to_string_literal(&dataurl)?
    }
    ModuleType::Binary => {
      let source = source.into_bytes();
      let encoded = to_standard_base64(source);
      binary_to_esm(&encoded, options.platform, RUNTIME_MODULE_ID)
    }
    ModuleType::Asset => "import.meta.__ROLLDOWN_ASSET_FILENAME".to_string(),
    ModuleType::Empty => String::new(),
    ModuleType::Custom(custom_type) => {
      return Err(anyhow::format_err!("Unknown module type: {custom_type}"))?;
    }
  };

  Ok((has_lazy_export, source.into(), module_type.into()))
}
