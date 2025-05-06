use minipack_common::{ImportKind, ImportRecordMeta};
use minipack_utils::option_ext::OptionExt;
use oxc::{
  ast::ast::{self, IdentifierReference},
  ast_visit::{Visit, walk},
  span::GetSpan,
};

use super::{AstScanner, side_effect_detector::SideEffectDetector};

impl<'me, 'ast: 'me> Visit<'ast> for AstScanner<'me, 'ast> {
  fn enter_node(&mut self, kind: oxc::ast::AstKind<'ast>) {
    self.visit_path.push(kind);
  }

  fn leave_node(&mut self, _: oxc::ast::AstKind<'ast>) {
    self.visit_path.pop();
  }

  fn visit_program(&mut self, program: &ast::Program<'ast>) {
    for (idx, stmt) in program.body.iter().enumerate() {
      self.current_stmt_info.stmt_idx = Some(idx.into());
      self.current_stmt_info.side_effect =
        SideEffectDetector::new(&self.result.symbols.ast_scopes).detect_side_effect_of_stmt(stmt);

      self.visit_statement(stmt);
      self.result.stmt_infos.add_stmt_info(std::mem::take(&mut self.current_stmt_info));
    }
  }

  fn visit_binding_identifier(&mut self, ident: &ast::BindingIdentifier) {
    let symbol_id = ident.symbol_id.get().unpack();
    if self.is_root_symbol(symbol_id) {
      self.add_declared_id(symbol_id);
    }
  }

  fn visit_identifier_reference(&mut self, ident: &IdentifierReference) {
    self.process_identifier_ref_by_scope(ident);
    self.try_diagnostic_forbid_const_assign(ident);
  }

  fn visit_statement(&mut self, stmt: &ast::Statement<'ast>) {
    if let Some(decl) = stmt.as_module_declaration() {
      self.scan_module_decl(decl);
    }
    walk::walk_statement(self, stmt);
  }

  fn visit_import_expression(&mut self, expr: &ast::ImportExpression<'ast>) {
    if let ast::Expression::StringLiteral(request) = &expr.source {
      let import_rec_idx = self.add_import_record(
        request.value.as_str(),
        ImportKind::DynamicImport,
        expr.source.span(),
        if expr.source.span().is_empty() {
          ImportRecordMeta::IS_UNSPANNED_IMPORT
        } else {
          ImportRecordMeta::empty()
        },
      );
      self.result.imports.insert(expr.span, import_rec_idx);
    }
    walk::walk_import_expression(self, expr);
  }
}
