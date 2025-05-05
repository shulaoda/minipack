use itertools::Itertools;
use minipack_ecmascript::AstSnippet;
use minipack_ecmascript::StatementExt;
use oxc::allocator::Allocator;
use oxc::allocator::TakeIn;
use oxc::ast::NONE;
use oxc::ast::ast::{self, BindingPatternKind, Declaration, ImportOrExportKind, Statement};
use oxc::ast_visit::{VisitMut, walk_mut};
use oxc::span::{SPAN, Span};

/// Pre-process is a essential step to make rolldown generate correct and efficient code.
pub struct PreProcessor<'ast> {
  snippet: AstSnippet<'ast>,
  /// For top level statements, this is used to store none_hoisted statements.
  /// For none top level statements, this is used to store split `VarDeclaration`.
  stmt_temp_storage: Vec<Statement<'ast>>,
  need_push_ast: bool,
  pub contains_use_strict: bool,
}

impl<'ast> PreProcessor<'ast> {
  pub fn new(alloc: &'ast Allocator) -> Self {
    Self {
      snippet: AstSnippet::new(alloc),
      contains_use_strict: false,
      stmt_temp_storage: vec![],
      need_push_ast: false,
    }
  }

  /// split `var a = 1, b = 2;` into `var a = 1; var b = 2;`
  fn split_var_declaration(
    &self,
    var_decl: &mut ast::VariableDeclaration<'ast>,
    named_decl_span: Option<Span>,
  ) -> Vec<Statement<'ast>> {
    var_decl
      .declarations
      .take_in(self.snippet.alloc())
      .into_iter()
      .enumerate()
      .map(|(i, declarator)| {
        let new_decl = self.snippet.builder.alloc_variable_declaration(
          SPAN,
          var_decl.kind,
          self.snippet.builder.vec_from_iter([declarator]),
          var_decl.declare,
        );
        if let Some(named_decl_span) = named_decl_span {
          Statement::ExportNamedDeclaration(self.snippet.builder.alloc_export_named_declaration(
            if i == 0 { named_decl_span } else { SPAN },
            Some(Declaration::VariableDeclaration(new_decl)),
            self.snippet.builder.vec(),
            // Since it is `export a = 1, b = 2;`, source should be `None`
            None,
            ImportOrExportKind::Value,
            NONE,
          ))
        } else {
          Statement::VariableDeclaration(new_decl)
        }
      })
      .collect_vec()
  }
}

impl<'ast> VisitMut<'ast> for PreProcessor<'ast> {
  fn visit_program(&mut self, program: &mut ast::Program<'ast>) {
    let directives_len = program.directives.len();
    program.directives.retain(|directive| !directive.is_use_strict());
    self.contains_use_strict = directives_len != program.directives.len();

    let drain_elements = program
      .body
      .drain_filter(|stmt| !stmt.is_module_declaration_with_source())
      .collect::<Vec<_>>();

    self.stmt_temp_storage = Vec::with_capacity(drain_elements.len());
    for mut stmt in drain_elements {
      self.need_push_ast = true;
      walk_mut::walk_statement(self, &mut stmt);
      if self.need_push_ast {
        self.stmt_temp_storage.push(stmt);
      }
    }
    program.body.extend(std::mem::take(&mut self.stmt_temp_storage));
  }

  fn visit_export_named_declaration(&mut self, named_decl: &mut ast::ExportNamedDeclaration<'ast>) {
    walk_mut::walk_export_named_declaration(self, named_decl);

    let Some(Declaration::VariableDeclaration(ref mut var_decl)) = named_decl.declaration else {
      return;
    };

    if var_decl
      .declarations
      .iter()
      .any(|declarator| matches!(declarator.id.kind, BindingPatternKind::BindingIdentifier(_)))
    {
      let rewritten = self.split_var_declaration(var_decl, Some(named_decl.span));
      self.stmt_temp_storage.extend(rewritten);
      self.need_push_ast = false;
    }
  }

  fn visit_expression(&mut self, it: &mut ast::Expression<'ast>) {
    let to_replaced = match it {
      // transpose `import(test ? 'a' : 'b')` into `test ? import('a') : import('b')`
      ast::Expression::ImportExpression(expr) if expr.options.is_none() => {
        let source = &mut expr.source;
        match source {
          ast::Expression::ConditionalExpression(cond_expr) => {
            let test = cond_expr.test.take_in(self.snippet.alloc());
            let consequent = cond_expr.consequent.take_in(self.snippet.alloc());
            let alternative = cond_expr.alternate.take_in(self.snippet.alloc());

            let new_cond_expr = self.snippet.builder.expression_conditional(
              SPAN,
              test,
              self.snippet.builder.expression_import(SPAN, consequent, None, None),
              self.snippet.builder.expression_import(SPAN, alternative, None, None),
            );

            Some(new_cond_expr)
          }
          _ => None,
        }
      }
      _ => None,
    };
    if let Some(replaced) = to_replaced {
      *it = replaced;
    }
    walk_mut::walk_expression(self, it);
  }
}
