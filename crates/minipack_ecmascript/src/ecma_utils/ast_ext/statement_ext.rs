use oxc::ast::ast;

pub trait StatementExt<'me, 'ast> {
  fn is_import_declaration(&self) -> bool;
  fn is_export_all_declaration(&self) -> bool;
  fn is_module_declaration_with_source(&self) -> bool;

  fn as_export_default_declaration_mut(
    &'me mut self,
  ) -> Option<&'me mut ast::ExportDefaultDeclaration<'ast>>;
  fn as_export_named_declaration_mut(&mut self) -> Option<&mut ast::ExportNamedDeclaration<'ast>>;
}

impl<'ast> StatementExt<'_, 'ast> for ast::Statement<'ast> {
  fn is_import_declaration(&self) -> bool {
    matches!(self, ast::Statement::ImportDeclaration(_))
  }

  fn is_export_all_declaration(&self) -> bool {
    matches!(self, ast::Statement::ExportAllDeclaration(_))
  }

  fn as_export_default_declaration_mut(
    &mut self,
  ) -> Option<&mut ast::ExportDefaultDeclaration<'ast>> {
    if let ast::Statement::ExportDefaultDeclaration(export_default_decl) = self {
      return Some(&mut **export_default_decl);
    }
    None
  }

  fn as_export_named_declaration_mut(&mut self) -> Option<&mut ast::ExportNamedDeclaration<'ast>> {
    if let ast::Statement::ExportNamedDeclaration(export_named_decl) = self {
      return Some(&mut **export_named_decl);
    }
    None
  }

  /// Check if the statement is `[import|export] ... from ...` or `export ... from ...`
  fn is_module_declaration_with_source(&self) -> bool {
    matches!(self.as_module_declaration(), Some(decl) if decl.source().is_some())
  }
}
