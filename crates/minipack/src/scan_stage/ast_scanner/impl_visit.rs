use minipack_common::{
  ImportKind, ImportRecordMeta, StmtInfoMeta, dynamic_import_usage::DynamicImportExportsUsage,
};
use minipack_utils::option_ext::OptionExt;
use oxc::{
  ast::{
    AstKind, Visit,
    ast::{self, BindingPatternKind, IdentifierReference},
    visit::walk,
  },
  semantic::SymbolId,
  span::GetSpan,
};

use super::{AstScanner, side_effect_detector::SideEffectDetector};

impl<'me, 'ast: 'me> Visit<'ast> for AstScanner<'me, 'ast> {
  fn enter_scope(
    &mut self,
    _flags: oxc::semantic::ScopeFlags,
    scope_id: &std::cell::Cell<Option<oxc::semantic::ScopeId>>,
  ) {
    self.scope_stack.push(scope_id.get());
  }

  fn leave_scope(&mut self) {
    self.scope_stack.pop();
  }

  fn enter_node(&mut self, kind: oxc::ast::AstKind<'ast>) {
    self.visit_path.push(kind);
  }

  fn leave_node(&mut self, _: oxc::ast::AstKind<'ast>) {
    self.visit_path.pop();
  }

  fn visit_program(&mut self, program: &ast::Program<'ast>) {
    for (idx, stmt) in program.body.iter().enumerate() {
      self.current_stmt_info.stmt_idx = Some(idx.into());
      self.current_stmt_info.side_effect = SideEffectDetector::new(
        &self.result.scopes,
        self.source,
        self.comments,
        true,
        &self.result.symbols,
      )
      .detect_side_effect_of_stmt(stmt);

      self.visit_statement(stmt);
      self.result.stmt_infos.add_stmt_info(std::mem::take(&mut self.current_stmt_info));
    }

    self.result.hashbang_range = program.hashbang.as_ref().map(GetSpan::span);
    self.result.dynamic_import_rec_exports_usage =
      std::mem::take(&mut self.dynamic_import_usage_info.dynamic_import_exports_usage);
    if self.result.has_eval {
      // if there exists `eval` in current module, assume all dynamic import are completely used;
      for usage in self.result.dynamic_import_rec_exports_usage.values_mut() {
        *usage = DynamicImportExportsUsage::Complete;
      }
    }
  }

  fn visit_binding_identifier(&mut self, ident: &ast::BindingIdentifier) {
    let symbol_id = ident.symbol_id.get().unpack();
    if self.is_root_symbol(symbol_id) {
      self.add_declared_id(symbol_id);
    }
  }

  fn visit_for_of_statement(&mut self, it: &ast::ForOfStatement<'ast>) {
    let is_top_level_await = it.r#await && self.is_valid_tla_scope();
    if is_top_level_await && !self.options.format.is_esm() {
      self.result.errors.push(anyhow::anyhow!(
        "Top-level await is currently not supported with the '{format}' output format",
        format = self.options.format
      ));
    }
    if is_top_level_await {
      self.result.has_top_level_await = true
    }
    walk::walk_for_of_statement(self, it);
  }

  fn visit_await_expression(&mut self, it: &ast::AwaitExpression<'ast>) {
    let is_top_level_await = self.is_valid_tla_scope();
    if !self.options.format.is_esm() && is_top_level_await {
      self.result.errors.push(anyhow::anyhow!(
        "Top-level await is currently not supported with the '{format}' output format",
        format = self.options.format
      ));
    }
    if is_top_level_await {
      self.result.has_top_level_await = true;
    }
    walk::walk_await_expression(self, it);
  }

  fn visit_identifier_reference(&mut self, ident: &IdentifierReference) {
    self.process_identifier_ref_by_scope(ident);
    self.try_diagnostic_forbid_const_assign(ident);
    self.update_dynamic_import_binding_usage_info(ident);
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
      self.init_dynamic_import_binding_usage_info(import_rec_idx);
      self.result.imports.insert(expr.span, import_rec_idx);
    }
    walk::walk_import_expression(self, expr);
  }

  fn visit_this_expression(&mut self, it: &ast::ThisExpression) {
    if !self.is_this_nested() {
      self.result.this_expr_replace_map.insert(it.span);
    }
    walk::walk_this_expression(self, it);
  }

  fn visit_class(&mut self, it: &ast::Class<'ast>) {
    let previous_class_decl_id = self.cur_class_decl.take();
    self.cur_class_decl = self.get_class_id(it);
    walk::walk_class(self, it);
    self.cur_class_decl = previous_class_decl_id;
  }

  fn visit_class_element(&mut self, it: &ast::ClassElement<'ast>) {
    let pre_is_nested_this_inside_class = self.is_nested_this_inside_class;
    self.is_nested_this_inside_class = true;
    walk::walk_class_element(self, it);
    self.is_nested_this_inside_class = pre_is_nested_this_inside_class;
  }

  fn visit_property_key(&mut self, it: &ast::PropertyKey<'ast>) {
    let pre_is_nested_this_inside_class = self.is_nested_this_inside_class;
    if let Some(AstKind::ClassBody(_)) = self.visit_path.iter().rev().nth(1) {
      self.is_nested_this_inside_class = false;
    }
    walk::walk_property_key(self, it);
    self.is_nested_this_inside_class = pre_is_nested_this_inside_class;
  }

  fn visit_declaration(&mut self, it: &ast::Declaration<'ast>) {
    match it {
      ast::Declaration::VariableDeclaration(decl) => {
        if let [decl] = decl.declarations.as_slice() {
          if let (BindingPatternKind::BindingIdentifier(_), Some(init)) =
            (&decl.id.kind, &decl.init)
          {
            match init {
              ast::Expression::ClassExpression(_) => {
                self.current_stmt_info.meta.insert(StmtInfoMeta::ClassExpr);
              }
              ast::Expression::FunctionExpression(_) => {
                self.current_stmt_info.meta.insert(StmtInfoMeta::FnExpr);
              }
              _ => {}
            }
          }
        }
      }
      ast::Declaration::FunctionDeclaration(_) => {
        self.current_stmt_info.meta.insert(StmtInfoMeta::FnDecl);
      }
      ast::Declaration::ClassDeclaration(_) => {
        self.current_stmt_info.meta.insert(StmtInfoMeta::ClassDecl);
      }
      _ => {}
    }
    walk::walk_declaration(self, it);
  }
}

impl<'me, 'ast: 'me> AstScanner<'me, 'ast> {
  /// visit `Class` of declaration
  pub fn get_class_id(&mut self, class: &ast::Class<'ast>) -> Option<SymbolId> {
    let id = class.id.as_ref()?;
    let symbol_id = *id.symbol_id.get().unpack_ref();
    Some(symbol_id)
  }

  fn process_identifier_ref_by_scope(&mut self, ident_ref: &IdentifierReference) {
    match self.resolve_identifier_reference(ident_ref) {
      super::IdentifierReferenceKind::Global => {
        self.process_global_identifier_ref_by_ancestor(ident_ref);
      }
      super::IdentifierReferenceKind::Root(root_symbol_id) => {
        // if the identifier_reference is a NamedImport MemberExpr access, we store it as a `MemberExpr`
        // use this flag to avoid insert it as `Symbol` at the same time.
        let mut is_inserted_before = false;
        if self.result.named_imports.contains_key(&root_symbol_id) {
          if let Some((span, props)) = self.try_extract_parent_static_member_expr_chain(usize::MAX)
          {
            if !span.is_unspanned() {
              is_inserted_before = true;
              self.add_member_expr_reference(root_symbol_id, props, span);
            }
          }
        }
        if !is_inserted_before {
          self.add_referenced_symbol(root_symbol_id);
        }

        self.check_import_assign(ident_ref, root_symbol_id.symbol);

        match (self.cur_class_decl, self.resolve_symbol_from_reference(ident_ref)) {
          (Some(cur_class_decl), Some(referenced_to)) if cur_class_decl == referenced_to => {
            self.result.self_referenced_class_decl_symbol_ids.insert(cur_class_decl);
          }
          _ => {}
        }
      }
      super::IdentifierReferenceKind::Other => {}
    };
  }

  fn process_global_identifier_ref_by_ancestor(
    &mut self,
    ident_ref: &IdentifierReference,
  ) -> Option<()> {
    let parent = self.visit_path.last()?;
    if let AstKind::CallExpression(_) = parent {
      if ident_ref.name == "eval" {
        self.result.has_eval = true;
        self.result.warnings.push(
          anyhow::anyhow!("Use of eval in '{}' is strongly discouraged as it poses security risks and may cause issues with minification.", self.id.as_ref())
        );
      }
    }
    None
  }
}
