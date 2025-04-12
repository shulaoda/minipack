use oxc::{
  allocator::{self, Allocator, Box, Dummy, IntoIn, TakeIn},
  ast::{
    AstBuilder, NONE,
    ast::{
      self, Argument, BindingIdentifier, ClassElement, Declaration, Expression, ImportOrExportKind,
      NumberBase, ObjectPropertyKind, PropertyKind, Statement, VariableDeclarationKind,
    },
  },
  span::{Atom, CompactStr, SPAN, Span},
};

type PassedStr<'a> = &'a str;

// `AstBuilder` is more suitable name, but it's already used in oxc.
pub struct AstSnippet<'ast> {
  pub builder: AstBuilder<'ast>,
}

impl<'ast> AstSnippet<'ast> {
  pub fn new(alloc: &'ast Allocator) -> Self {
    Self { builder: AstBuilder::new(alloc) }
  }

  #[inline]
  pub fn alloc(&self) -> &'ast Allocator {
    self.builder.allocator
  }

  pub fn atom(&self, value: &str) -> Atom<'ast> {
    self.builder.atom(value)
  }

  #[inline]
  pub fn id(&self, name: PassedStr, span: Span) -> ast::BindingIdentifier<'ast> {
    self.builder.binding_identifier(span, name)
  }

  #[inline]
  pub fn alloc_id_ref(
    &self,
    name: PassedStr,
    span: Span,
  ) -> Box<'ast, ast::IdentifierReference<'ast>> {
    self.builder.alloc_identifier_reference(span, name)
  }

  #[inline]
  pub fn id_name(&self, name: PassedStr, span: Span) -> ast::IdentifierName<'ast> {
    self.builder.identifier_name(span, name)
  }

  #[inline]
  pub fn id_ref_expr(&self, name: PassedStr, span: Span) -> ast::Expression<'ast> {
    self.builder.expression_identifier(span, name)
  }

  pub fn member_expr_or_ident_ref(
    &self,
    object: ast::Expression<'ast>,
    names: &[CompactStr],
    span: Span,
  ) -> ast::Expression<'ast> {
    match names {
      [] => object,
      _ => ast::Expression::StaticMemberExpression(self.builder.alloc_static_member_expression(
        span,
        self.member_expr_or_ident_ref(object, &names[0..names.len() - 1], span),
        self.id_name(names[names.len() - 1].as_str(), span),
        false,
      )),
    }
  }

  /// The props of `foo_exports.value.a` is `["value", "a"]`, here convert it to `(void 0).a`
  pub fn member_expr_with_void_zero_object(
    &self,
    names: &[CompactStr],
    span: Span,
  ) -> ast::Expression<'ast> {
    if names.len() == 1 {
      self.void_zero()
    } else {
      ast::Expression::StaticMemberExpression(self.builder.alloc_static_member_expression(
        span,
        self.member_expr_with_void_zero_object(&names[0..names.len() - 1], span),
        self.id_name(names[names.len() - 1].as_str(), span),
        false,
      ))
    }
  }

  /// `[object].[property]`
  pub fn literal_prop_access_member_expr(
    &self,
    object: PassedStr,
    property: PassedStr,
  ) -> ast::MemberExpression<'ast> {
    ast::MemberExpression::StaticMemberExpression(self.builder.alloc_static_member_expression(
      SPAN,
      self.id_ref_expr(object, SPAN),
      self.builder.identifier_name(SPAN, property),
      false,
    ))
  }

  /// `[object].[property]`
  #[inline]
  pub fn literal_prop_access_member_expr_expr(
    &self,
    object: PassedStr,
    property: PassedStr,
  ) -> ast::Expression<'ast> {
    ast::Expression::from(self.literal_prop_access_member_expr(object, property))
  }

  /// `name()`
  #[inline]
  pub fn call_expr(&self, name: PassedStr) -> ast::CallExpression<'ast> {
    self.builder.call_expression(
      SPAN,
      self.builder.expression_identifier(SPAN, name),
      NONE,
      self.builder.vec(),
      false,
    )
  }

  /// `name()`
  pub fn call_expr_expr(&self, name: PassedStr) -> ast::Expression<'ast> {
    self.builder.expression_call(
      SPAN,
      self.builder.expression_identifier(SPAN, name),
      NONE,
      self.builder.vec(),
      false,
    )
  }

  /// `name(arg)`
  pub fn call_expr_with_arg_expr(
    &self,
    name: ast::Expression<'ast>,
    arg: ast::Expression<'ast>,
  ) -> ast::Expression<'ast> {
    let mut call_expr = self.simple_call_expr(name);
    call_expr.arguments.push(arg.into());
    ast::Expression::CallExpression(call_expr.into_in(self.alloc()))
  }

  /// `name(arg)`
  pub fn call_expr_with_arg_expr_expr(
    &self,
    name: PassedStr,
    arg: ast::Expression<'ast>,
  ) -> ast::Expression<'ast> {
    let arg = ast::Argument::from(arg);
    let mut call_expr = self.call_expr(name);
    call_expr.arguments.push(arg);
    ast::Expression::CallExpression(call_expr.into_in(self.alloc()))
  }

  /// `name(arg1, arg2)`
  pub fn call_expr_with_2arg_expr(
    &self,
    name: ast::Expression<'ast>,
    arg1: ast::Expression<'ast>,
    arg2: ast::Expression<'ast>,
  ) -> ast::Expression<'ast> {
    let mut call_expr = self.builder.call_expression(SPAN, name, NONE, self.builder.vec(), false);
    call_expr.arguments.push(arg1.into());
    call_expr.arguments.push(arg2.into());
    ast::Expression::CallExpression(call_expr.into_in(self.alloc()))
  }

  /// `name(arg1, arg2)`
  pub fn alloc_call_expr_with_2arg_expr_expr(
    &self,
    name: PassedStr,
    arg1: ast::Expression<'ast>,
    arg2: ast::Expression<'ast>,
  ) -> ast::Expression<'ast> {
    self.builder.expression_call(
      SPAN,
      self.builder.expression_identifier(SPAN, name),
      NONE,
      self.builder.vec_from_iter([Argument::from(arg1), Argument::from(arg2)]),
      false,
    )
  }

  /// `name()`
  #[inline]
  pub fn call_expr_stmt(&self, name: PassedStr) -> ast::Statement<'ast> {
    self.builder.statement_expression(SPAN, self.call_expr_expr(name))
  }

  /// `var [name] = [init]`
  #[inline]
  pub fn var_decl_stmt(
    &self,
    name: PassedStr,
    init: ast::Expression<'ast>,
  ) -> ast::Statement<'ast> {
    ast::Statement::from(self.decl_var_decl(name, init))
  }

  /// `var [name] = [init]`
  pub fn decl_var_decl(
    &self,
    name: PassedStr,
    init: ast::Expression<'ast>,
  ) -> ast::Declaration<'ast> {
    let declarations = self.builder.vec1(self.builder.variable_declarator(
      SPAN,
      ast::VariableDeclarationKind::Var,
      self.builder.binding_pattern(
        self.builder.binding_pattern_kind_binding_identifier(SPAN, name),
        NONE,
        false,
      ),
      Some(init),
      false,
    ));

    ast::Declaration::VariableDeclaration(self.builder.alloc_variable_declaration(
      SPAN,
      ast::VariableDeclarationKind::Var,
      declarations,
      false,
    ))
  }

  /// ```js
  /// (a, b)
  /// ```
  pub fn seq2_in_paren_expr(
    &self,
    a: ast::Expression<'ast>,
    b: ast::Expression<'ast>,
  ) -> ast::Expression<'ast> {
    let mut expressions = self.builder.vec_with_capacity(2);
    expressions.push(a);
    expressions.push(b);
    let seq_expr = ast::Expression::SequenceExpression(
      self.builder.alloc_sequence_expression(SPAN, expressions),
    );
    ast::Expression::ParenthesizedExpression(
      self.builder.alloc_parenthesized_expression(SPAN, seq_expr),
    )
  }

  pub fn number_expr(&self, value: f64, raw: &'ast str) -> ast::Expression<'ast> {
    ast::Expression::NumericLiteral(self.builder.alloc_numeric_literal(
      SPAN,
      value,
      Some(Atom::from(raw)),
      oxc::syntax::number::NumberBase::Decimal,
    ))
  }

  /// ```js
  ///  id = ...
  /// ￣￣ AssignmentTarget
  /// ```
  pub fn simple_id_assignment_target(
    &self,
    id: PassedStr,
    span: Span,
  ) -> ast::AssignmentTarget<'ast> {
    ast::AssignmentTarget::AssignmentTargetIdentifier(self.alloc_id_ref(id, span))
  }

  /// ```js
  /// () => xx
  /// ```
  pub fn only_return_arrow_expr(&self, expr: ast::Expression<'ast>) -> ast::Expression<'ast> {
    let statements = self.builder.vec1(ast::Statement::ExpressionStatement(
      self.builder.alloc_expression_statement(SPAN, expr),
    ));
    ast::Expression::ArrowFunctionExpression(self.builder.alloc_arrow_function_expression(
      SPAN,
      true,
      false,
      NONE,
      self.builder.formal_parameters(
        SPAN,
        ast::FormalParameterKind::Signature,
        self.builder.vec(),
        NONE,
      ),
      NONE,
      self.builder.function_body(SPAN, self.builder.vec(), statements),
    ))
  }

  #[inline]
  /// `undefined` is acting like identifier, it might be shadowed by user code.
  pub fn void_zero(&self) -> ast::Expression<'ast> {
    self.builder.void_0(SPAN)
  }

  pub fn alloc_string_literal(
    &self,
    value: PassedStr,
    span: Span,
  ) -> Box<'ast, ast::StringLiteral<'ast>> {
    self.builder.alloc_string_literal(span, value, None)
  }

  pub fn string_literal_expr(&self, value: PassedStr, span: Span) -> ast::Expression<'ast> {
    ast::Expression::StringLiteral(self.alloc_string_literal(value, span))
  }

  pub fn import_star_stmt(&self, source: PassedStr, as_name: PassedStr) -> ast::Statement<'ast> {
    let specifiers = self.builder.vec1(ast::ImportDeclarationSpecifier::ImportNamespaceSpecifier(
      self.builder.alloc_import_namespace_specifier(SPAN, self.id(as_name, SPAN)),
    ));
    ast::Statement::ImportDeclaration(self.builder.alloc_import_declaration(
      SPAN,
      Some(specifiers),
      self.builder.string_literal(SPAN, source, None),
      None,
      NONE,
      ImportOrExportKind::Value,
    ))
  }

  pub fn require_call_expr(&self, source: &str) -> Expression<'ast> {
    self.builder.expression_call(
      SPAN,
      self.builder.expression_identifier(SPAN, "require"),
      NONE,
      self.builder.vec1(Argument::from(self.builder.expression_string_literal(SPAN, source, None))),
      false,
    )
  }

  /// `var [assignee] = require([source]);`
  pub fn variable_declarator_require_call_stmt(
    &self,
    assignee: &str,
    init: ast::Expression<'ast>,
    span: Span,
  ) -> Statement<'ast> {
    Statement::from(self.builder.declaration_variable(
      span,
      VariableDeclarationKind::Var,
      self.builder.vec1(self.builder.variable_declarator(
        SPAN,
        VariableDeclarationKind::Var,
        self.builder.binding_pattern(
          self.builder.binding_pattern_kind_binding_identifier(SPAN, assignee),
          NONE,
          false,
        ),
        Some(init),
        false,
      )),
      false,
    ))
  }

  /// Promise.resolve().then(function() {})
  pub fn promise_resolve_then_call_expr(
    &self,
    span: Span,
    statements: allocator::Vec<'ast, Statement<'ast>>,
  ) -> ast::Expression<'ast> {
    let arguments = self.builder.vec1(Argument::FunctionExpression(self.builder.alloc_function(
      SPAN,
      ast::FunctionType::FunctionExpression,
      None::<BindingIdentifier>,
      false,
      false,
      false,
      NONE,
      NONE,
      self.builder.formal_parameters(
        SPAN,
        ast::FormalParameterKind::Signature,
        self.builder.vec_with_capacity(2),
        NONE,
      ),
      NONE,
      Some(self.builder.function_body(SPAN, self.builder.vec(), statements)),
    )));

    let callee =
      ast::Expression::StaticMemberExpression(self.builder.alloc_static_member_expression(
        SPAN,
        ast::Expression::CallExpression(self.builder.alloc_call_expression(
          SPAN,
          ast::Expression::StaticMemberExpression(self.builder.alloc_static_member_expression(
            SPAN,
            self.id_ref_expr("Promise", SPAN),
            self.id_name("resolve", SPAN),
            false,
          )),
          NONE,
          self.builder.vec(),
          false,
        )),
        self.id_name("then", SPAN),
        false,
      ));
    ast::Expression::CallExpression(
      self.builder.alloc_call_expression(span, callee, NONE, arguments, false),
    )
  }

  // return xxx
  pub fn return_stmt(&self, argument: ast::Expression<'ast>) -> ast::Statement<'ast> {
    ast::Statement::ReturnStatement(
      ast::ReturnStatement {
        argument: Some(argument),
        ..ast::ReturnStatement::dummy(self.alloc())
      }
      .into_in(self.alloc()),
    )
  }

  // create `a: () => expr` for  `{ a: () => expr }``
  pub fn object_property_kind_object_property(
    &self,
    key: PassedStr,
    expr: ast::Expression<'ast>,
    computed: bool,
  ) -> ObjectPropertyKind<'ast> {
    self.builder.object_property_kind_object_property(
      SPAN,
      PropertyKind::Init,
      if computed {
        ast::PropertyKey::from(self.builder.expression_string_literal(SPAN, key, None))
      } else {
        self.builder.property_key_static_identifier(SPAN, key)
      },
      self.only_return_arrow_expr(expr),
      true,
      false,
      computed,
    )
  }

  // If `node_mode` is true, using `__toESM(expr, 1)`
  // If `node_mode` is false, using `__toESM(expr)`
  pub fn wrap_with_to_esm(
    &self,
    to_esm_fn_expr: Expression<'ast>,
    expr: Expression<'ast>,
    node_mode: bool,
  ) -> Expression<'ast> {
    let args = if node_mode {
      self.builder.vec_from_iter([
        Argument::from(expr),
        Argument::from(self.builder.expression_numeric_literal(
          SPAN,
          1.0,
          None,
          NumberBase::Decimal,
        )),
      ])
    } else {
      self.builder.vec1(Argument::from(expr))
    };
    ast::Expression::CallExpression(self.builder.alloc_call_expression(
      SPAN,
      to_esm_fn_expr,
      NONE,
      args,
      false,
    ))
  }

  /// convert `Expression` to
  /// export default ${Expression}
  pub fn export_default_expr_stmt(&self, expr: Expression<'ast>) -> Statement<'ast> {
    let ast_builder = &self.builder;
    Statement::from(ast_builder.module_declaration_export_default_declaration(
      SPAN,
      ast_builder.module_export_name_identifier_name(SPAN, "default"),
      ast::ExportDefaultDeclarationKind::from(expr),
    ))
  }

  /// convert `Expression` to
  /// module.exports = ${Expression}
  pub fn module_exports_expr_stmt(&self, expr: Expression<'ast>) -> Statement<'ast> {
    let ast_builder = &self.builder;
    ast_builder.statement_expression(
      SPAN,
      ast_builder.expression_assignment(
        SPAN,
        ast::AssignmentOperator::Assign,
        ast::AssignmentTarget::from(ast::SimpleAssignmentTarget::from(
          ast_builder.member_expression_static(
            SPAN,
            ast_builder.expression_identifier(SPAN, "module"),
            ast_builder.identifier_name(SPAN, "exports"),
            false,
          ),
        )),
        expr,
      ),
    )
  }

  pub fn expr_without_parentheses(&self, mut expr: Expression<'ast>) -> Expression<'ast> {
    while let Expression::ParenthesizedExpression(mut paren_expr) = expr {
      expr = paren_expr.expression.take_in(self.builder.allocator);
    }
    expr
  }

  #[inline]
  pub fn statement_module_declaration_export_named_declaration<T: AsRef<str>>(
    &self,
    declaration: Option<Declaration<'ast>>,
    specifiers: &[(T /*local*/, T /*exported*/, bool /*legal ident*/)],
  ) -> Statement<'ast> {
    Statement::from(self.builder.module_declaration_export_named_declaration(
      SPAN,
      declaration,
      {
        let mut vec = self.builder.vec_with_capacity(specifiers.len());
        for (local, exported, legal_ident) in specifiers {
          vec.push(self.builder.export_specifier(
            SPAN,
            self.builder.module_export_name_identifier_reference(SPAN, local.as_ref()),
            if *legal_ident {
              self.builder.module_export_name_identifier_name(SPAN, exported.as_ref())
            } else {
              self.builder.module_export_name_string_literal(SPAN, exported.as_ref(), None)
            },
            ImportOrExportKind::Value,
          ));
        }
        vec
      },
      None,
      ImportOrExportKind::Value,
      NONE,
    ))
  }

  pub fn keep_name_call_expr_stmt(
    &self,
    original_name: PassedStr,
    new_name: PassedStr,
  ) -> Statement<'ast> {
    self.builder.statement_expression(
      SPAN,
      self.builder.expression_call(
        SPAN,
        self.builder.expression_identifier(SPAN, "__name"),
        NONE,
        {
          let mut items = self.builder.vec_with_capacity(2);
          items.push(self.builder.expression_identifier(SPAN, new_name).into());
          items.push(self.builder.expression_string_literal(SPAN, original_name, None).into());
          items
        },
        false,
      ),
    )
  }

  pub fn static_block_keep_name_helper(&self, name: PassedStr) -> ClassElement<'ast> {
    self.builder.class_element_static_block(
      SPAN,
      self.builder.vec1(self.builder.statement_expression(
        SPAN,
        self.builder.expression_call(
          SPAN,
          self.builder.expression_identifier(SPAN, "__name"),
          NONE,
          {
            let mut items = self.builder.vec_with_capacity(2);
            items.push(self.builder.expression_this(SPAN).into());
            items.push(self.builder.expression_string_literal(SPAN, name, None).into());
            items
          },
          false,
        ),
      )),
    )
  }

  pub fn simple_call_expr(&self, callee: Expression<'ast>) -> ast::CallExpression<'ast> {
    self.builder.call_expression(SPAN, callee, NONE, self.builder.vec(), false)
  }

  pub fn alloc_simple_call_expr(
    &self,
    callee: Expression<'ast>,
  ) -> allocator::Box<'ast, ast::CallExpression<'ast>> {
    self.builder.alloc_call_expression(SPAN, callee, NONE, self.builder.vec(), false)
  }
}
