use oxc::syntax::identifier;

use crate::concat_string;

pub fn is_validate_identifier_name(name: &str) -> bool {
  identifier::is_identifier_name(name)
}

pub fn property_access_str(obj: &str, prop: &str) -> String {
  if is_validate_identifier_name(prop) {
    concat_string!(obj, ".", prop)
  } else {
    concat_string!(obj, "[", serde_json::to_string(prop).unwrap(), "]")
  }
}

pub fn to_module_import_export_name(name: &str) -> String {
  if is_validate_identifier_name(name) { name.into() } else { serde_json::to_string(name).unwrap() }
}

#[test]
fn test_is_validate_identifier_name() {
  assert!(is_validate_identifier_name("foo"));
  assert!(!is_validate_identifier_name("1aaaa"));
  assert!(!is_validate_identifier_name("😈"));
}
