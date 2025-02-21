use std::fmt::Display;

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum ImportKind {
  /// import foo from 'foo'
  Import,
  /// `import('foo')`
  DynamicImport,
  /// css @import
  AtImport,
  /// css url import, e.g. `url(foo.png)`
  UrlImport,
  // `new URL('path', import.meta.url)`
  NewUrl,
}

impl ImportKind {
  pub fn is_static(&self) -> bool {
    matches!(self, Self::Import | Self::AtImport | Self::UrlImport)
  }
}

impl TryFrom<&str> for ImportKind {
  type Error = String;

  fn try_from(value: &str) -> Result<Self, Self::Error> {
    match value {
      "import" => Ok(Self::Import),
      "dynamic-import" => Ok(Self::DynamicImport),
      "import-rule" => Ok(Self::AtImport),
      "url-import" => Ok(Self::UrlImport),
      _ => Err(format!("Invalid import kind: {value:?}")),
    }
  }
}

impl Display for ImportKind {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    // https://github.com/evanw/esbuild/blob/d34e79e2a998c21bb71d57b92b0017ca11756912/internal/ast/ast.go#L42
    match self {
      Self::Import => write!(f, "import-statement"),
      Self::DynamicImport => write!(f, "dynamic-import"),
      Self::AtImport => write!(f, "import-rule"),
      Self::UrlImport => write!(f, "url-import"),
      Self::NewUrl => write!(f, "new-url"),
    }
  }
}
