use oxc::ast::ast;

pub trait ExpressionExt<'ast> {
  fn is_import_meta(&self) -> bool;

  fn as_string_literal(&self) -> Option<&ast::StringLiteral<'ast>>;
  fn as_identifier(&self) -> Option<&ast::IdentifierReference<'ast>>;
  fn as_identifier_mut(&mut self) -> Option<&mut ast::IdentifierReference<'ast>>;
  fn as_unary_expression(&self) -> Option<&ast::UnaryExpression<'ast>>;
  fn as_binary_expression(&self) -> Option<&ast::BinaryExpression<'ast>>;
}

impl<'ast> ExpressionExt<'ast> for ast::Expression<'ast> {
  fn is_import_meta(&self) -> bool {
    matches!(self, ast::Expression::MetaProperty(meta_prop)
    if meta_prop.meta.name == "import" && meta_prop.property.name == "meta")
  }

  fn as_identifier(&self) -> Option<&ast::IdentifierReference<'ast>> {
    if let ast::Expression::Identifier(ident) = self { Some(ident) } else { None }
  }

  fn as_identifier_mut(&mut self) -> Option<&mut ast::IdentifierReference<'ast>> {
    if let ast::Expression::Identifier(ident) = self { Some(ident) } else { None }
  }

  fn as_unary_expression(&self) -> Option<&ast::UnaryExpression<'ast>> {
    let ast::Expression::UnaryExpression(expr) = self else {
      return None;
    };
    Some(expr)
  }

  fn as_string_literal(&self) -> Option<&ast::StringLiteral<'ast>> {
    let ast::Expression::StringLiteral(expr) = self else {
      return None;
    };
    Some(expr)
  }

  fn as_binary_expression(&self) -> Option<&ast::BinaryExpression<'ast>> {
    let ast::Expression::BinaryExpression(expr) = self else {
      return None;
    };
    Some(expr)
  }
}
