/// Render namespace markers for the module.
/// It contains the `__esModule` and `Symbol.toStringTag` properties.
/// Since rolldown doesn't support `generatedCode.symbol` yet,
/// it's not possible to use `Symbol.toStringTag` in the output.
pub fn render_namespace_markers(
  has_default_export: bool,
  namespace_to_string_tag: bool,
) -> Option<&'static str> {
  if has_default_export {
    if namespace_to_string_tag {
      Some("Object.defineProperties(exports, { __esModule: { value: true }, [Symbol.toStringTag]: { value: 'Module' } });")
    } else {
      Some("Object.defineProperty(exports, '__esModule', { value: true });")
    }
  } else if namespace_to_string_tag {
    Some("Object.defineProperty(exports, Symbol.toStringTag, { value: 'Module' });")
  } else {
    None
  }
}
