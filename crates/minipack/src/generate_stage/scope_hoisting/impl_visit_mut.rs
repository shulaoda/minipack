use minipack_common::{Module, StmtInfoIdx};
use minipack_ecmascript::ExpressionExt;
use oxc::{
  allocator::IntoIn,
  ast::ast::{self, SimpleAssignmentTarget},
  ast_visit::{VisitMut, walk_mut},
  span::Span,
};
use rustc_hash::FxHashSet;

impl<'ast> VisitMut<'ast> for super::ScopeHoistingFinalizer<'_, 'ast> {
  fn visit_program(&mut self, program: &mut ast::Program<'ast>) {
    self.namespace_alias_symbol_id = self
      .ctx
      .module
      .ecma_view
      .named_imports
      .iter()
      .filter_map(|(symbol_ref, v)| {
        let rec_id = v.record_id;
        let importee_idx = self.ctx.module.ecma_view.import_records[rec_id].state;
        self.ctx.modules[importee_idx].as_normal()?;
        self.ctx.symbol_ref_db.get(*symbol_ref).namespace_alias.as_ref().and_then(|alias| {
          (alias.property_name.as_str() == "default").then_some(symbol_ref.symbol)
        })
      })
      .collect::<FxHashSet<_>>();

    self.remove_unused_top_level_stmt(program);

    if self.ctx.module.stmt_infos[StmtInfoIdx::new(0)].is_included {
      let stmts = self.generate_declaration_of_module_namespace_object();
      program.body.splice(0..0, stmts);
    }

    walk_mut::walk_program(self, program);
  }

  fn visit_binding_identifier(&mut self, ident: &mut ast::BindingIdentifier<'ast>) {
    if let Some(symbol_id) = ident.symbol_id.get() {
      let canonical_name = self.canonical_name_for((self.ctx.id, symbol_id).into());
      if ident.name != canonical_name {
        ident.name = self.snippet.atom(canonical_name);
      }
      ident.symbol_id.get_mut().take();
    }
  }

  fn visit_call_expression(&mut self, expr: &mut ast::CallExpression<'ast>) {
    if let Some(new_expr) = expr
      .callee
      .as_identifier_mut()
      .and_then(|ident_ref| self.try_rewrite_identifier_reference_expr(ident_ref, true))
    {
      expr.callee = new_expr;
    }
    walk_mut::walk_call_expression(self, expr);
  }

  fn visit_expression(&mut self, expr: &mut ast::Expression<'ast>) {
    match expr {
      ast::Expression::ImportExpression(import_expr) => {
        if let Some(new_expr) = self.try_rewrite_inline_dynamic_import_expr(import_expr) {
          *expr = new_expr;
        }
      }
      ast::Expression::Identifier(ident_ref) => {
        if let Some(new_expr) = self.try_rewrite_identifier_reference_expr(ident_ref, false) {
          *expr = new_expr;
        }
      }
      _ => {
        if let Some(new_expr) =
          expr.as_member_expression().and_then(|expr| self.try_rewrite_member_expr(expr))
        {
          *expr = new_expr;
        }
      }
    };
    walk_mut::walk_expression(self, expr);
  }

  // foo.js `export const bar = { a: 0 }`
  // main.js `import * as foo_exports from './foo.js';\n foo_exports.bar.a = 1;`
  // The `foo_exports.bar.a` ast is `StaticMemberExpression(StaticMemberExpression)`,
  // The outer StaticMemberExpression span is `foo_exports.bar.a`,
  // the `visit_expression(Expression::MemberExpression)` is called with `foo_exports.bar`,
  // the span is inner StaticMemberExpression.
  fn visit_member_expression(&mut self, expr: &mut ast::MemberExpression<'ast>) {
    if let Some(new_expr) = self.try_rewrite_member_expr(expr) {
      *expr = new_expr.into_member_expression();
    } else {
      walk_mut::walk_member_expression(self, expr);
    }
  }

  fn visit_object_property(&mut self, prop: &mut ast::ObjectProperty<'ast>) {
    // Ensure `{ a }` would be rewritten to `{ a: a$1 }` instead of `{ a$1 }`
    if prop.shorthand {
      if let ast::Expression::Identifier(id_ref) = &mut prop.value {
        if let Some(expr) = self.generate_finalized_expr_for_reference(id_ref, false) {
          prop.value = expr;
          prop.shorthand = false;
        } else {
          id_ref.reference_id.get_mut().take();
        }
      }
    }
    walk_mut::walk_object_property(self, prop);
  }

  fn visit_object_pattern(&mut self, pat: &mut ast::ObjectPattern<'ast>) {
    self.rewrite_object_pat_shorthand(pat);
    walk_mut::walk_object_pattern(self, pat);
  }

  fn visit_import_expression(&mut self, expr: &mut ast::ImportExpression<'ast>) {
    if expr.options.is_none() {
      // Make sure the import expression is in correct form. If it's not, we should leave it as it is.
      if let ast::Expression::StringLiteral(str) = &mut expr.source {
        let rec_id = self.ctx.module.imports[&expr.span];
        let rec = &self.ctx.module.import_records[rec_id];
        let importee_id = rec.state;
        match &self.ctx.modules[importee_id] {
          Module::Normal(_importee) => {
            let importer_chunk = &self.ctx.chunk_graph.chunk_table[self.ctx.chunk_id];

            let importee_chunk_id = self.ctx.chunk_graph.entry_module_to_chunk[&importee_id];
            let importee_chunk = &self.ctx.chunk_graph.chunk_table[importee_chunk_id];

            let import_path = importer_chunk.import_path_for(importee_chunk);

            str.value = self.snippet.atom(&import_path);
          }
          Module::External(importee) => {
            if str.value != importee.name {
              str.value = self.snippet.atom(&importee.name);
            }
          }
        }
      }
    }
    walk_mut::walk_import_expression(self, expr);
  }

  fn visit_assignment_target_property(
    &mut self,
    property: &mut ast::AssignmentTargetProperty<'ast>,
  ) {
    if let ast::AssignmentTargetProperty::AssignmentTargetPropertyIdentifier(prop) = property {
      if let Some(target) = self.generate_finalized_simple_assignment_target(&prop.binding) {
        *property = ast::AssignmentTargetProperty::AssignmentTargetPropertyProperty(
          ast::AssignmentTargetPropertyProperty {
            name: ast::PropertyKey::StaticIdentifier(
              self.snippet.id_name(&prop.binding.name, prop.span).into_in(self.allocator),
            ),
            binding: if let Some(init) = prop.init.take() {
              ast::AssignmentTargetMaybeDefault::AssignmentTargetWithDefault(
                ast::AssignmentTargetWithDefault {
                  binding: ast::AssignmentTarget::from(target),
                  init,
                  span: Span::default(),
                }
                .into_in(self.allocator),
              )
            } else {
              ast::AssignmentTargetMaybeDefault::from(target)
            },
            span: Span::default(),
            computed: false,
          }
          .into_in(self.allocator),
        );
      } else {
        prop.binding.reference_id.get_mut().take();
      }
    }

    walk_mut::walk_assignment_target_property(self, property);
  }

  fn visit_simple_assignment_target(&mut self, target: &mut SimpleAssignmentTarget<'ast>) {
    self.rewrite_simple_assignment_target(target);
    walk_mut::walk_simple_assignment_target(self, target);
  }
}
