use minipack_common::EsModuleFlag;

/// Render namespace markers for the module.
/// It contains the `__esModule` and `Symbol.toStringTag` properties.
/// Since rolldown doesn't support `generatedCode.symbol` yet,
/// it's not possible to use `Symbol.toStringTag` in the output.
pub fn render_namespace_markers(
  es_module_flag: EsModuleFlag,
  has_default_export: bool,
  // TODO namespace_to_string_tag
  namespace_to_string_tag: bool,
) -> Option<&'static str> {
  let es_module = match es_module_flag {
    EsModuleFlag::Always => true,
    EsModuleFlag::IfDefaultProp if has_default_export => true,
    _ => false,
  };

  if es_module && namespace_to_string_tag {
    Some("Object.defineProperties(exports, { __esModule: { value: true }, [Symbol.toStringTag]: { value: 'Module' } });")
  } else if es_module {
    Some("Object.defineProperty(exports, '__esModule', { value: true });")
  } else if namespace_to_string_tag {
    Some("Object.defineProperty(exports, Symbol.toStringTag, { value: 'Module' });")
  } else {
    None
  }
}
