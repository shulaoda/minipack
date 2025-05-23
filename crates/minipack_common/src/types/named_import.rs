use std::fmt::Display;

use minipack_utils::rstr::Rstr;
use oxc::span::Span;

use crate::{ImportRecordIdx, SymbolRef};

/// This is a representation for statements like
/// - Case A: `import { foo } from 'foo'`
/// - Case B: `import * as fooNs from 'foo'`
/// - Case C: `import { foo as foo2 } from 'foo'`
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NamedImport {
  /// For case A, the `imported` is `foo`.
  /// For case B, the `imported` is meaningless.
  /// For case C, the `imported` is `foo`.
  pub imported: Specifier,
  pub imported_span: Span,
  /// For case A, the `imported_as` is a `SymbolRef` from `foo`.
  /// For case B, the `imported_as` is a `SymbolRef` from `fooNs`.
  /// For case C, the `imported_as` is a `SymbolRef` from `foo2`.
  pub imported_as: SymbolRef,
  pub record_id: ImportRecordIdx,
}

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub enum Specifier {
  Star,
  Literal(Rstr),
}

impl Display for Specifier {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      Self::Star => "*".fmt(f),
      Self::Literal(atom) => atom.as_str().fmt(f),
    }
  }
}

impl From<Rstr> for Specifier {
  fn from(atom: Rstr) -> Self {
    Self::Literal(atom)
  }
}

impl From<&str> for Specifier {
  fn from(s: &str) -> Self {
    Self::Literal(Rstr::from(s))
  }
}
