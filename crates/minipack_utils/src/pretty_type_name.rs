use std::borrow::Cow;

use regex::Regex;
use std::sync::LazyLock;

static MODULE_MATCHER_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?:\w+::)").unwrap());

pub fn pretty_type_name<T: ?Sized>() -> Cow<'static, str> {
  let type_name = std::any::type_name::<T>();
  MODULE_MATCHER_RE.replace_all(type_name, "")
}

#[test]
fn test_pretty_type_name() {
  struct Custom;
  assert_eq!(pretty_type_name::<std::option::Option<std::string::String>>(), "Option<String>");
  assert_eq!(pretty_type_name::<std::option::Option<Custom>>(), "Option<Custom>");
}
