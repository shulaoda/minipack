mod utils;

use minipack_common::AstScopes;
use minipack_utils::global_reference::{
  is_global_ident_ref, is_side_effect_free_member_expr_of_len_three,
  is_side_effect_free_member_expr_of_len_two,
};
use oxc::ast::ast::{
  self, Argument, ArrayExpressionElement, AssignmentTarget, AssignmentTargetPattern,
  BindingPatternKind, ChainElement, Expression, IdentifierReference, PropertyKey, UnaryOperator,
  VariableDeclarationKind,
};
use oxc::ast::{match_expression, match_member_expression};

use utils::{
  PrimitiveType, can_change_strict_to_loose, extract_member_expr_chain, is_primitive_literal,
  is_side_effect_free_unbound_identifier_ref, known_primitive_type,
  maybe_side_effect_free_global_constructor,
};

pub struct SideEffectDetector<'a> {
  pub scope: &'a AstScopes,
}

impl<'a> SideEffectDetector<'a> {
  pub fn new(scope: &'a AstScopes) -> Self {
    Self { scope }
  }

  fn is_unresolved_reference(&self, ident_ref: &IdentifierReference) -> bool {
    self.scope.is_unresolved(ident_ref.reference_id.get().unwrap())
  }

  fn detect_side_effect_of_property_key(&self, key: &PropertyKey, is_computed: bool) -> bool {
    match key {
      PropertyKey::StaticIdentifier(_) | PropertyKey::PrivateIdentifier(_) => false,
      key @ oxc::ast::match_expression!(PropertyKey) => {
        is_computed && {
          let key_expr = key.to_expression();
          match key_expr {
            match_member_expression!(Expression) => {
              if let Some((ref_id, chain)) =
                extract_member_expr_chain(key_expr.to_member_expression(), 2)
              {
                !(chain == ["Symbol", "iterator"] && self.scope.is_unresolved(ref_id))
              } else {
                true
              }
            }
            _ => !is_primitive_literal(self.scope, key_expr),
          }
        }
      }
    }
  }

  /// ref: https://github.com/evanw/esbuild/blob/360d47230813e67d0312ad754cad2b6ee09b151b/internal/js_ast/js_ast_helpers.go#L2298-L2393
  fn detect_side_effect_of_class(&self, cls: &ast::Class) -> bool {
    use oxc::ast::ast::ClassElement;
    if !cls.decorators.is_empty() {
      return true;
    }
    cls.body.body.iter().any(|elm| match elm {
      ClassElement::StaticBlock(static_block) => {
        static_block.body.iter().any(|stmt| self.detect_side_effect_of_stmt(stmt))
      }
      ClassElement::MethodDefinition(def) => {
        if !def.decorators.is_empty() {
          return true;
        }
        if self.detect_side_effect_of_property_key(&def.key, def.computed) {
          return true;
        }

        def.value.params.items.iter().any(|item| !item.decorators.is_empty())
      }
      ClassElement::PropertyDefinition(def) => {
        if !def.decorators.is_empty() {
          return true;
        }
        if self.detect_side_effect_of_property_key(&def.key, def.computed) {
          return true;
        }

        def.r#static && def.value.as_ref().is_some_and(|init| self.detect_side_effect_of_expr(init))
      }
      ClassElement::AccessorProperty(def) => {
        (match &def.key {
          PropertyKey::StaticIdentifier(_) | PropertyKey::PrivateIdentifier(_) => false,
          key @ oxc::ast::match_expression!(PropertyKey) => {
            self.detect_side_effect_of_expr(key.to_expression())
          }
        } || def.value.as_ref().is_some_and(|init| self.detect_side_effect_of_expr(init)))
      }
      ClassElement::TSIndexSignature(_) => unreachable!("ts should be transpiled"),
    })
  }

  fn detect_side_effect_of_member_expr(&self, expr: &ast::MemberExpression) -> bool {
    // MemberExpression is considered having side effect by default, unless it's some builtin global variables.
    let Some((ref_id, chains)) = extract_member_expr_chain(expr, 3) else {
      return true;
    };
    // If the global variable is override, we considered it has side effect.
    if !self.scope.is_unresolved(ref_id) {
      return true;
    }
    match chains.len() {
      2 => !is_side_effect_free_member_expr_of_len_two(&chains),
      3 => !is_side_effect_free_member_expr_of_len_three(&chains),
      _ => true,
    }
  }

  fn detect_side_effect_of_assignment_target(expr: &AssignmentTarget) -> bool {
    let Some(pattern) = expr.as_assignment_target_pattern() else {
      return true;
    };
    match pattern {
      // {} = expr
      AssignmentTargetPattern::ArrayAssignmentTarget(array_pattern) => {
        !array_pattern.elements.is_empty() || array_pattern.rest.is_some()
      }
      // [] = expr
      AssignmentTargetPattern::ObjectAssignmentTarget(object_pattern) => {
        !object_pattern.properties.is_empty() || object_pattern.rest.is_some()
      }
    }
  }

  fn detect_side_effect_of_expr(&self, expr: &Expression) -> bool {
    match expr {
      Expression::BooleanLiteral(_)
      | Expression::NullLiteral(_)
      | Expression::NumericLiteral(_)
      | Expression::BigIntLiteral(_)
      | Expression::RegExpLiteral(_)
      | Expression::FunctionExpression(_)
      | Expression::ArrowFunctionExpression(_)
      | Expression::MetaProperty(_)
      | Expression::ThisExpression(_)
      | Expression::StringLiteral(_) => false,
      Expression::ObjectExpression(obj_expr) => {
        obj_expr.properties.iter().any(|obj_prop| match obj_prop {
          ast::ObjectPropertyKind::ObjectProperty(prop) => {
            let key_side_effect = self.detect_side_effect_of_property_key(&prop.key, prop.computed);
            if key_side_effect {
              return true;
            }
            self.detect_side_effect_of_expr(&prop.value)
          }
          ast::ObjectPropertyKind::SpreadProperty(_) => {
            // ...[expression] is considered as having side effect.
            true
          }
        })
      }
      // https://github.com/evanw/esbuild/blob/d34e79e2a998c21bb71d57b92b0017ca11756912/internal/js_ast/js_ast_helpers.go#L2533-L2539
      Expression::UnaryExpression(unary_expr) => match unary_expr.operator {
        ast::UnaryOperator::Typeof if matches!(unary_expr.argument, Expression::Identifier(_)) => {
          false
        }
        _ => self.detect_side_effect_of_expr(&unary_expr.argument),
      },
      oxc::ast::match_member_expression!(Expression) => {
        self.detect_side_effect_of_member_expr(expr.to_member_expression())
      }
      Expression::ClassExpression(cls) => self.detect_side_effect_of_class(cls),
      // Accessing global variables considered as side effect.
      Expression::Identifier(ident) => self.detect_side_effect_of_identifier(ident),
      // https://github.com/evanw/esbuild/blob/360d47230813e67d0312ad754cad2b6ee09b151b/internal/js_ast/js_ast_helpers.go#L2576-L2588
      Expression::TemplateLiteral(literal) => literal.expressions.iter().any(|expr| {
        // Primitive type detection is more strict and faster than side_effects detection of
        // `Expr`, put it first to fail fast.
        known_primitive_type(self.scope, expr) == PrimitiveType::Unknown
          || self.detect_side_effect_of_expr(expr)
      }),
      Expression::LogicalExpression(logic_expr) => match logic_expr.operator {
        ast::LogicalOperator::Or => {
          self.detect_side_effect_of_expr(&logic_expr.left)
            || (!is_side_effect_free_unbound_identifier_ref(
              self.scope,
              &logic_expr.right,
              &logic_expr.left,
              false,
            )
            .unwrap_or_default()
              && self.detect_side_effect_of_expr(&logic_expr.right))
        }
        ast::LogicalOperator::And => {
          self.detect_side_effect_of_expr(&logic_expr.left)
            || (!is_side_effect_free_unbound_identifier_ref(
              self.scope,
              &logic_expr.right,
              &logic_expr.left,
              true,
            )
            .unwrap_or_default()
              && self.detect_side_effect_of_expr(&logic_expr.right))
        }
        ast::LogicalOperator::Coalesce => {
          self.detect_side_effect_of_expr(&logic_expr.left)
            || self.detect_side_effect_of_expr(&logic_expr.right)
        }
      },
      Expression::ParenthesizedExpression(paren_expr) => {
        self.detect_side_effect_of_expr(&paren_expr.expression)
      }
      Expression::SequenceExpression(seq_expr) => {
        seq_expr.expressions.iter().any(|expr| self.detect_side_effect_of_expr(expr))
      }
      // https://github.com/evanw/esbuild/blob/d34e79e2a998c21bb71d57b92b0017ca11756912/internal/js_ast/js_ast_helpers.go#L2460-L2463
      Expression::ConditionalExpression(cond_expr) => {
        self.detect_side_effect_of_expr(&cond_expr.test)
          || (!is_side_effect_free_unbound_identifier_ref(
            self.scope,
            &cond_expr.consequent,
            &cond_expr.test,
            true,
          )
          .unwrap_or_default()
            && self.detect_side_effect_of_expr(&cond_expr.consequent))
          || (!is_side_effect_free_unbound_identifier_ref(
            self.scope,
            &cond_expr.alternate,
            &cond_expr.test,
            false,
          )
          .unwrap_or_default()
            && self.detect_side_effect_of_expr(&cond_expr.alternate))
      }
      Expression::TSAsExpression(_)
      | Expression::TSSatisfiesExpression(_)
      | Expression::TSTypeAssertion(_)
      | Expression::TSNonNullExpression(_)
      | Expression::TSInstantiationExpression(_) => unreachable!("ts should be transpiled"),
      // https://github.com/evanw/esbuild/blob/d34e79e2a998c21bb71d57b92b0017ca11756912/internal/js_ast/js_ast_helpers.go#L2541-L2574
      Expression::BinaryExpression(binary_expr) => match binary_expr.operator {
        ast::BinaryOperator::StrictEquality | ast::BinaryOperator::StrictInequality => {
          self.detect_side_effect_of_expr(&binary_expr.left)
            || self.detect_side_effect_of_expr(&binary_expr.right)
        }
        // Special-case "<" and ">" with string, number, or bigint arguments
        ast::BinaryOperator::GreaterThan
        | ast::BinaryOperator::LessThan
        | ast::BinaryOperator::GreaterEqualThan
        | ast::BinaryOperator::LessEqualThan => {
          let lt = known_primitive_type(self.scope, &binary_expr.left);
          match lt {
            PrimitiveType::Number | PrimitiveType::String | PrimitiveType::BigInt => {
              known_primitive_type(self.scope, &binary_expr.right) != lt
                || self.detect_side_effect_of_expr(&binary_expr.left)
                || self.detect_side_effect_of_expr(&binary_expr.right)
            }
            _ => true,
          }
        }

        // For "==" and "!=", pretend the operator was actually "===" or "!==". If
        // we know that we can convert it to "==" or "!=", then we can consider the
        // operator itself to have no side effects. This matters because our mangle
        // logic will convert "typeof x === 'object'" into "typeof x == 'object'"
        // and since "typeof x === 'object'" is considered to be side-effect free,
        // we must also consider "typeof x == 'object'" to be side-effect free.
        ast::BinaryOperator::Equality | ast::BinaryOperator::Inequality => {
          !can_change_strict_to_loose(self.scope, &binary_expr.left, &binary_expr.right)
            || self.detect_side_effect_of_expr(&binary_expr.left)
            || self.detect_side_effect_of_expr(&binary_expr.right)
        }
        _ => true,
      },
      Expression::PrivateInExpression(private_in_expr) => {
        self.detect_side_effect_of_expr(&private_in_expr.right)
      }
      Expression::AssignmentExpression(expr) => {
        Self::detect_side_effect_of_assignment_target(&expr.left)
          || self.detect_side_effect_of_expr(&expr.right)
      }
      Expression::ChainExpression(expr) => match &expr.expression {
        ChainElement::CallExpression(_) => true,
        ChainElement::TSNonNullExpression(expr) => {
          self.detect_side_effect_of_expr(&expr.expression)
        }
        match_member_expression!(ChainElement) => {
          self.detect_side_effect_of_member_expr(expr.expression.to_member_expression())
        }
      },
      Expression::Super(_)
      | Expression::AwaitExpression(_)
      | Expression::ImportExpression(_)
      | Expression::TaggedTemplateExpression(_)
      | Expression::UpdateExpression(_)
      | Expression::YieldExpression(_)
      | Expression::V8IntrinsicExpression(_) => true,
      Expression::JSXElement(_) | Expression::JSXFragment(_) => {
        unreachable!("jsx should be transpiled")
      }
      Expression::ArrayExpression(expr) => self.detect_side_effect_of_array_expr(expr),
      Expression::NewExpression(expr) => {
        if maybe_side_effect_free_global_constructor(self.scope, expr) {
          expr.arguments.iter().any(|arg| match arg {
            Argument::SpreadElement(_) => true,
            _ => self.detect_side_effect_of_expr(arg.to_expression()),
          })
        } else {
          true
        }
      }
      Expression::CallExpression(_) => true,
    }
  }

  fn detect_side_effect_of_array_expr(&self, expr: &ast::ArrayExpression<'_>) -> bool {
    expr.elements.iter().any(|elem| match elem {
      ArrayExpressionElement::SpreadElement(ele) => {
        // https://github.com/evanw/esbuild/blob/d34e79e2a998c21bb71d57b92b0017ca11756912/internal/js_ast/js_ast_helpers.go#L2466-L2477
        // Spread of an inline array such as "[...[x]]" is side-effect free
        match &ele.argument {
          Expression::ArrayExpression(arr) => self.detect_side_effect_of_array_expr(arr),
          _ => true,
        }
      }
      ArrayExpressionElement::Elision(_) => false,
      match_expression!(ArrayExpressionElement) => {
        self.detect_side_effect_of_expr(elem.to_expression())
      }
    })
  }

  fn detect_side_effect_of_var_decl(&self, var_decl: &ast::VariableDeclaration) -> bool {
    match var_decl.kind {
      VariableDeclarationKind::AwaitUsing => true,
      VariableDeclarationKind::Using => {
        self.detect_side_effect_of_using_declarators(&var_decl.declarations)
      }
      _ => var_decl.declarations.iter().any(|declarator| {
        // Whether to destructure import.meta
        if let BindingPatternKind::ObjectPattern(ref obj_pat) = declarator.id.kind {
          if !obj_pat.properties.is_empty() {
            if let Some(Expression::MetaProperty(_)) = declarator.init {
              return true;
            }
          }
        }
        match &declarator.id.kind {
          // Destructuring the initializer has no side effects if the
          // initializer is an array, since we assume the iterator is then
          // the built-in side-effect free array iterator.
          BindingPatternKind::ObjectPattern(_) => true,
          BindingPatternKind::ArrayPattern(pat) => {
            for p in &pat.elements {
              match &p {
                Some(binding_pat)
                  if matches!(binding_pat.kind, BindingPatternKind::BindingIdentifier(_)) =>
                {
                  continue;
                }
                None => continue,
                _ => {
                  return true;
                }
              }
            }
            declarator.init.as_ref().is_some_and(|init| self.detect_side_effect_of_expr(init))
          }
          BindingPatternKind::BindingIdentifier(_) | BindingPatternKind::AssignmentPattern(_) => {
            declarator.init.as_ref().is_some_and(|init| self.detect_side_effect_of_expr(init))
          }
        }
      }),
    }
  }

  fn detect_side_effect_of_decl(&self, decl: &ast::Declaration) -> bool {
    use oxc::ast::ast::Declaration;
    match decl {
      Declaration::VariableDeclaration(var_decl) => self.detect_side_effect_of_var_decl(var_decl),
      Declaration::FunctionDeclaration(_) => false,
      Declaration::ClassDeclaration(cls_decl) => self.detect_side_effect_of_class(cls_decl),
      Declaration::TSTypeAliasDeclaration(_)
      | Declaration::TSInterfaceDeclaration(_)
      | Declaration::TSEnumDeclaration(_)
      | Declaration::TSModuleDeclaration(_)
      | Declaration::TSImportEqualsDeclaration(_) => unreachable!("ts should be transpiled"),
    }
  }

  fn detect_side_effect_of_using_declarators(
    &self,
    declarators: &[ast::VariableDeclarator],
  ) -> bool {
    declarators.iter().any(|decl| {
      decl.init.as_ref().is_some_and(|init| match init {
        Expression::NullLiteral(_) => false,
        // Side effect detection of identifier is different with other position when as initialization of using declaration.
        // Global variable `undefined` is considered as side effect free.
        Expression::Identifier(id) => !(id.name == "undefined" && self.is_unresolved_reference(id)),
        Expression::UnaryExpression(expr) if matches!(expr.operator, UnaryOperator::Void) => {
          self.detect_side_effect_of_expr(&expr.argument)
        }
        _ => true,
      })
    })
  }

  #[inline]
  fn detect_side_effect_of_identifier(&self, ident_ref: &IdentifierReference) -> bool {
    self.is_unresolved_reference(ident_ref) && !is_global_ident_ref(&ident_ref.name)
  }

  pub fn detect_side_effect_of_stmt(&self, stmt: &ast::Statement) -> bool {
    use oxc::ast::ast::Statement;
    match stmt {
      oxc::ast::match_declaration!(Statement) => {
        self.detect_side_effect_of_decl(stmt.to_declaration())
      }
      Statement::ExpressionStatement(expr) => self.detect_side_effect_of_expr(&expr.expression),
      oxc::ast::match_module_declaration!(Statement) => match stmt.to_module_declaration() {
        ast::ModuleDeclaration::ExportAllDeclaration(_)
        | ast::ModuleDeclaration::ImportDeclaration(_) => {
          // We consider `import ...` has no side effect. However, `import ...` might be rewritten to other statements by the bundler.
          // In that case, we will mark the statement as having side effect in link stage.
          false
        }
        ast::ModuleDeclaration::ExportDefaultDeclaration(default_decl) => {
          use oxc::ast::ast::ExportDefaultDeclarationKind;
          match &default_decl.declaration {
            decl @ oxc::ast::match_expression!(ExportDefaultDeclarationKind) => {
              self.detect_side_effect_of_expr(decl.to_expression())
            }
            ast::ExportDefaultDeclarationKind::FunctionDeclaration(_) => false,
            ast::ExportDefaultDeclarationKind::ClassDeclaration(decl) => {
              self.detect_side_effect_of_class(decl)
            }
            ast::ExportDefaultDeclarationKind::TSInterfaceDeclaration(_) => {
              unreachable!("ts should be transpiled")
            }
          }
        }
        ast::ModuleDeclaration::ExportNamedDeclaration(named_decl) => {
          if named_decl.source.is_some() {
            false
          } else {
            named_decl
              .declaration
              .as_ref()
              .is_some_and(|decl| self.detect_side_effect_of_decl(decl))
          }
        }
        ast::ModuleDeclaration::TSExportAssignment(_)
        | ast::ModuleDeclaration::TSNamespaceExportDeclaration(_) => {
          unreachable!("ts should be transpiled")
        }
      },
      Statement::BlockStatement(block) => self.detect_side_effect_of_block(block),
      Statement::DoWhileStatement(do_while) => {
        self.detect_side_effect_of_stmt(&do_while.body)
          || self.detect_side_effect_of_expr(&do_while.test)
      }
      Statement::WhileStatement(while_stmt) => {
        self.detect_side_effect_of_expr(&while_stmt.test)
          || self.detect_side_effect_of_stmt(&while_stmt.body)
      }
      Statement::IfStatement(if_stmt) => {
        self.detect_side_effect_of_expr(&if_stmt.test)
          || self.detect_side_effect_of_stmt(&if_stmt.consequent)
          || if_stmt.alternate.as_ref().is_some_and(|stmt| self.detect_side_effect_of_stmt(stmt))
      }
      Statement::ReturnStatement(ret_stmt) => {
        ret_stmt.argument.as_ref().is_some_and(|expr| self.detect_side_effect_of_expr(expr))
      }
      Statement::LabeledStatement(labeled_stmt) => {
        self.detect_side_effect_of_stmt(&labeled_stmt.body)
      }
      Statement::TryStatement(try_stmt) => {
        self.detect_side_effect_of_block(&try_stmt.block)
          || try_stmt
            .handler
            .as_ref()
            .is_some_and(|handler| self.detect_side_effect_of_block(&handler.body))
          || try_stmt
            .finalizer
            .as_ref()
            .is_some_and(|finalizer| self.detect_side_effect_of_block(finalizer))
      }
      Statement::SwitchStatement(switch_stmt) => {
        self.detect_side_effect_of_expr(&switch_stmt.discriminant)
          || switch_stmt.cases.iter().any(|case| {
            case.test.as_ref().is_some_and(|expr| self.detect_side_effect_of_expr(expr))
              || case.consequent.iter().any(|stmt| self.detect_side_effect_of_stmt(stmt))
          })
      }
      Statement::EmptyStatement(_)
      | Statement::ContinueStatement(_)
      | Statement::BreakStatement(_) => false,
      Statement::DebuggerStatement(_)
      | Statement::ForInStatement(_)
      | Statement::ForOfStatement(_)
      | Statement::ForStatement(_)
      | Statement::ThrowStatement(_)
      | Statement::WithStatement(_) => true,
    }
  }

  fn detect_side_effect_of_block(&self, block: &ast::BlockStatement) -> bool {
    block.body.iter().any(|stmt| self.detect_side_effect_of_stmt(stmt))
  }
}
