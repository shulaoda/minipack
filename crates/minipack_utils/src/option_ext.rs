use std::sync::LazyLock;

use regex::Regex;

static MODULE_MATCHER_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?:\w+::)").unwrap());

pub trait OptionExt<T> {
  fn unpack(self) -> T;

  fn unpack_ref(&self) -> &T;

  fn unpack_ref_mut(&mut self) -> &mut T;
}

impl<T> OptionExt<T> for Option<T> {
  /// Similar to `unwrap`, but with a more descriptive panic message.
  ///
  /// ```ignore
  /// None::<usize>.unwrap();
  /// // called `Option::unwrap()` on a `None` value
  ///
  /// None::<usize>.unpack();
  /// // Got `None` value when calling `OptionExt::unpack()` on `Option<usize>`
  /// ```
  fn unpack(self) -> T {
    self.map_or_else(
      || {
        let type_name = std::any::type_name::<T>();
        let type_name = MODULE_MATCHER_RE.replace_all(type_name, "");
        panic!("Got `None` value when calling `OptionExt::unpack()` on `{type_name}`",)
      },
      |v| v,
    )
  }

  /// Shorthand for `self.as_ref().unpack()`.
  fn unpack_ref(&self) -> &T {
    self.as_ref().unpack()
  }

  /// Shorthand for `self.as_mut().unpack()`.
  fn unpack_ref_mut(&mut self) -> &mut T {
    self.as_mut().unpack()
  }
}
