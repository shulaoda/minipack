use std::path::Path;

use arcstr::ArcStr;
use minipack_common::{
  ModuleType, NormalizedBundlerOptions, Platform, StrOrBytes, RUNTIME_MODULE_ID,
};
use minipack_ecmascript::{EcmaAst, EcmaCompiler};
use minipack_error::BuildResult;
use minipack_utils::{
  base64::to_standard_base64, dataurl::encode_as_shortest_dataurl, mime::guess_mime,
  text_to_esm::text_to_string_literal,
};
use oxc::{
  semantic::{ScopeTree, SymbolTable},
  span::SourceType as OxcSourceType,
};

use super::pre_process_ecma_ast::PreProcessEcmaAst;

use crate::types::oxc_parse_type::OxcParseType;

fn pure_esm_js_oxc_source_type() -> OxcSourceType {
  let pure_esm_js = OxcSourceType::default().with_module(true);
  debug_assert!(pure_esm_js.is_javascript());
  debug_assert!(!pure_esm_js.is_jsx());
  debug_assert!(pure_esm_js.is_module());
  debug_assert!(pure_esm_js.is_strict());

  pure_esm_js
}

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
  pub symbol_table: SymbolTable,
  pub scope_tree: ScopeTree,
  pub has_lazy_export: bool,
  pub warning: Vec<anyhow::Error>,
}

#[allow(clippy::too_many_arguments)]
pub fn parse_to_ecma_ast(
  path: &Path,
  stable_id: &str,
  options: &NormalizedBundlerOptions,
  module_type: &ModuleType,
  source: StrOrBytes,
  is_user_defined_entry: bool,
) -> BuildResult<ParseToEcmaAstResult> {
  let (has_lazy_export, source, parsed_type) =
    pre_process_source(module_type, source, is_user_defined_entry, path, options)?;

  let oxc_source_type = {
    let default = pure_esm_js_oxc_source_type();
    match parsed_type {
      OxcParseType::Js | OxcParseType::Jsx => default,
      OxcParseType::Ts | OxcParseType::Tsx => default.with_typescript(true),
    }
  };

  let source = ArcStr::from(source);

  let ecma_ast = match module_type {
    ModuleType::Json | ModuleType::Dataurl | ModuleType::Base64 | ModuleType::Text => {
      EcmaCompiler::parse_expr_as_program(&source, oxc_source_type)?
    }
    _ => EcmaCompiler::parse(&source, oxc_source_type)?,
  };

  PreProcessEcmaAst::default().build(ecma_ast, &parsed_type, stable_id, options, has_lazy_export)
}

fn pre_process_source(
  module_type: &ModuleType,
  source: StrOrBytes,
  is_user_defined_entry: bool,
  path: &Path,
  options: &NormalizedBundlerOptions,
) -> BuildResult<(bool, String, OxcParseType)> {
  let mut has_lazy_export = false;
  let (source, parsed_type) = match module_type {
    ModuleType::Js => (source.try_into_string()?, OxcParseType::Js),
    ModuleType::Jsx => (source.try_into_string()?, OxcParseType::Jsx),
    ModuleType::Ts => (source.try_into_string()?, OxcParseType::Ts),
    ModuleType::Tsx => (source.try_into_string()?, OxcParseType::Tsx),
    ModuleType::Css => {
      if is_user_defined_entry {
        ("export {}".to_owned(), OxcParseType::Js)
      } else {
        has_lazy_export = true;
        ("({})".to_owned(), OxcParseType::Js)
      }
    }
    ModuleType::Json => {
      has_lazy_export = true;
      let content = source.try_into_string()?;
      (content, OxcParseType::Js)
    }
    ModuleType::Text => {
      let content = text_to_string_literal(&source.try_into_string()?)?;
      has_lazy_export = true;
      (content, OxcParseType::Js)
    }
    ModuleType::Base64 => {
      let source = source.into_bytes();
      let encoded = to_standard_base64(source);
      has_lazy_export = true;
      (text_to_string_literal(&encoded)?, OxcParseType::Js)
    }
    ModuleType::Dataurl => {
      let data = source.into_bytes();
      let guessed_mime = guess_mime(path, &data)?;
      let dataurl = encode_as_shortest_dataurl(&guessed_mime, &data);
      has_lazy_export = true;
      (text_to_string_literal(&dataurl)?, OxcParseType::Js)
    }
    ModuleType::Binary => {
      let source = source.into_bytes();
      let encoded = to_standard_base64(source);
      (binary_to_esm(&encoded, options.platform, RUNTIME_MODULE_ID), OxcParseType::Js)
    }
    ModuleType::Asset => {
      let content = "import.meta.__ROLLDOWN_ASSET_FILENAME".to_string();
      has_lazy_export = true;
      (content, OxcParseType::Js)
    }
    ModuleType::Empty => (String::new(), OxcParseType::Js),
    ModuleType::Custom(custom_type) => {
      // TODO: should provide friendly error message to say that this type is not supported by rolldown.
      // Users should handle this type in load/transform hooks
      return Err(anyhow::format_err!("Unknown module type: {custom_type}"))?;
    }
  };
  Ok((has_lazy_export, source, parsed_type))
}
