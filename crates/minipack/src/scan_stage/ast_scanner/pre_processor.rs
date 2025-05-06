use minipack_ecmascript::AstSnippet;
use minipack_ecmascript::StatementExt;
use oxc::allocator::Allocator;
use oxc::allocator::TakeIn;
use oxc::ast::NONE;
use oxc::ast::ast::{self, BindingPatternKind, Declaration, ImportOrExportKind, Statement};
use oxc::ast_visit::{VisitMut, walk_mut};
use oxc::span::SPAN;

pub struct PreProcessor<'ast> {
  snippet: AstSnippet<'ast>,
  need_push_ast: bool,
  stmt_temp_storage: Vec<Statement<'ast>>,
}

impl<'ast> PreProcessor<'ast> {
  pub fn new(alloc: &'ast Allocator) -> Self {
    Self { snippet: AstSnippet::new(alloc), stmt_temp_storage: vec![], need_push_ast: false }
  }
}

impl<'ast> VisitMut<'ast> for PreProcessor<'ast> {
  fn visit_program(&mut self, program: &mut ast::Program<'ast>) {
    let drain_elements = program
      .body
      .drain_filter(|stmt| !stmt.is_module_declaration_with_source())
      .collect::<Vec<_>>();

    self.stmt_temp_storage = Vec::with_capacity(drain_elements.len());
    for mut stmt in drain_elements {
      self.need_push_ast = true;
      self.visit_statement(&mut stmt);
      if self.need_push_ast {
        self.stmt_temp_storage.push(stmt);
      }
    }

    program.body.extend(std::mem::take(&mut self.stmt_temp_storage));
  }

  /// split `var a = 1, b = 2;` into `var a = 1; var b = 2;`
  fn visit_export_named_declaration(&mut self, named_decl: &mut ast::ExportNamedDeclaration<'ast>) {
    walk_mut::walk_export_named_declaration(self, named_decl);

    let Some(Declaration::VariableDeclaration(ref mut var_decl)) = named_decl.declaration else {
      return;
    };

    if var_decl.declarations.len() > 1
      && var_decl
        .declarations
        .iter()
        .any(|declarator| matches!(declarator.id.kind, BindingPatternKind::BindingIdentifier(_)))
    {
      self.need_push_ast = false;
      self.stmt_temp_storage.extend(
        var_decl.declarations.take_in(self.snippet.alloc()).into_iter().enumerate().map(
          |(i, declarator)| {
            let new_decl = self.snippet.builder.alloc_variable_declaration(
              SPAN,
              var_decl.kind,
              self.snippet.builder.vec_from_iter([declarator]),
              var_decl.declare,
            );
            Statement::ExportNamedDeclaration(self.snippet.builder.alloc_export_named_declaration(
              if i == 0 { named_decl.span } else { SPAN },
              Some(Declaration::VariableDeclaration(new_decl)),
              self.snippet.builder.vec(),
              // Since it is `export a = 1, b = 2;`, source should be `None`
              None,
              ImportOrExportKind::Value,
              NONE,
            ))
          },
        ),
      );
    }
  }

  /// transpose `import(test ? 'a' : 'b')` into `test ? import('a') : import('b')`
  fn visit_expression(&mut self, it: &mut ast::Expression<'ast>) {
    if let ast::Expression::ImportExpression(expr) = it {
      if expr.options.is_none() {
        if let ast::Expression::ConditionalExpression(cond_expr) = &mut expr.source {
          let new_cond_expr = self.snippet.builder.expression_conditional(
            SPAN,
            cond_expr.test.take_in(self.snippet.alloc()),
            self.snippet.builder.expression_import(
              SPAN,
              cond_expr.consequent.take_in(self.snippet.alloc()),
              None,
              None,
            ),
            self.snippet.builder.expression_import(
              SPAN,
              cond_expr.alternate.take_in(self.snippet.alloc()),
              None,
              None,
            ),
          );
          *it = new_cond_expr;
        }
      }
    }
    walk_mut::walk_expression(self, it);
  }
}
