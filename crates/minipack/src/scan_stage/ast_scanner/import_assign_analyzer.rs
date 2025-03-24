use minipack_common::Specifier;
use oxc::{
  ast::{
    AstKind,
    ast::{IdentifierReference, UnaryOperator},
  },
  semantic::{SymbolFlags, SymbolId},
};

use super::AstScanner;

impl<'me, 'ast: 'me> AstScanner<'me, 'ast> {
  pub fn check_import_assign(&mut self, ident: &IdentifierReference, symbol_id: SymbolId) {
    if self.result.symbols.symbol_flags(symbol_id).contains(SymbolFlags::Import) {
      let is_namespace = self
        .result
        .named_imports
        .get(&(self.idx, symbol_id).into())
        .is_some_and(|import| matches!(import.imported, Specifier::Star));
      if is_namespace {
        if let Some(name) = self.get_span_if_namespace_specifier_updated() {
          self.result.errors.push(anyhow::anyhow!("Cannot assign to import '{}'", name));
          return;
        }
      }
      if self.result.symbols.get_reference(ident.reference_id()).flags().is_write() {
        self.result.errors.push(anyhow::anyhow!("Cannot assign to import '{}'", ident.name));
      }
    }
  }

  fn get_span_if_namespace_specifier_updated(&mut self) -> Option<&'ast str> {
    let ancestor_cursor = self.visit_path.len() - 1;
    let parent_node = self.visit_path.get(ancestor_cursor)?;
    if let AstKind::MemberExpression(expr) = parent_node {
      let parent_parent_node = self.visit_path.get(ancestor_cursor - 1)?;
      let is_unary_expression_with_delete_operator = |kind| matches!(kind, AstKind::UnaryExpression(expr) if expr.operator == UnaryOperator::Delete);
      let parent_parent_kind = *parent_parent_node;
      if matches!(parent_parent_kind, AstKind::SimpleAssignmentTarget(_))
        // delete namespace.module
        || is_unary_expression_with_delete_operator(parent_parent_kind)
        // delete namespace?.module
        || matches!(parent_parent_kind, AstKind::ChainExpression(_) if self.visit_path.get(ancestor_cursor - 2).is_some_and(|item| {
          is_unary_expression_with_delete_operator(*item)
        }))
      {
        return expr.static_property_name();
      }
    }
    None
  }
}
