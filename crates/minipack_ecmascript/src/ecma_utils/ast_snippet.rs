use oxc::{
  allocator::{self, Allocator, Box, IntoIn},
  ast::{
    AstBuilder, NONE,
    ast::{self, Argument, BindingIdentifier, ImportOrExportKind, Statement},
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
    self.builder.binding_identifier(span, self.builder.atom(name))
  }
  
  #[inline]
  pub fn alloc_id_ref(
    &self,
    name: PassedStr,
    span: Span,
  ) -> Box<'ast, ast::IdentifierReference<'ast>> {
    self.builder.alloc_identifier_reference(span, self.builder.atom(name))
  }

  #[inline]
  pub fn id_name(&self, name: PassedStr, span: Span) -> ast::IdentifierName<'ast> {
    self.builder.identifier_name(span, self.builder.atom(name))
  }

  #[inline]
  pub fn id_ref_expr(&self, name: PassedStr, span: Span) -> ast::Expression<'ast> {
    self.builder.expression_identifier(span, self.builder.atom(name))
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
      self.builder.void_0(SPAN)
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
      self.builder.identifier_name(SPAN, self.builder.atom(property)),
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
      self.builder.expression_identifier(SPAN, self.builder.atom(name)),
      NONE,
      self.builder.vec(),
      false,
    )
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

  /// `var [name] = [init]`
  #[inline]
  pub fn var_decl_stmt(
    &self,
    name: PassedStr,
    init: ast::Expression<'ast>,
  ) -> ast::Statement<'ast> {
    let declarations = self.builder.vec1(self.builder.variable_declarator(
      SPAN,
      ast::VariableDeclarationKind::Var,
      self.builder.binding_pattern(
        self.builder.binding_pattern_kind_binding_identifier(SPAN, self.builder.atom(name)),
        NONE,
        false,
      ),
      Some(init),
      false,
    ));

    ast::Statement::from(ast::Declaration::VariableDeclaration(
      self.builder.alloc_variable_declaration(
        SPAN,
        ast::VariableDeclarationKind::Var,
        declarations,
        false,
      ),
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

  pub fn alloc_string_literal(
    &self,
    value: PassedStr,
    span: Span,
  ) -> Box<'ast, ast::StringLiteral<'ast>> {
    self.builder.alloc_string_literal(span, self.builder.atom(value), None)
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
      self.builder.string_literal(SPAN, self.builder.atom(source), None),
      None,
      NONE,
      ImportOrExportKind::Value,
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
}
