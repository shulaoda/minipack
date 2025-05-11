mod finalizer_context;
mod impl_visit_mut;
mod rename;

use minipack_common::{AstScopes, Module, OutputFormat, Platform, SymbolRef};
use minipack_ecmascript::{AstSnippet, ExpressionExt, StatementExt};
use minipack_utils::ecmascript::is_validate_identifier_name;
use oxc::{
  allocator::{Allocator, Box as ArenaBox, CloneIn, Dummy, IntoIn, TakeIn},
  ast::{
    NONE,
    ast::{self, ExportDefaultDeclarationKind, Expression, ImportExpression, MemberExpression},
  },
  semantic::SymbolId,
  span::{GetSpan, SPAN},
};
use rustc_hash::FxHashSet;

pub use finalizer_context::ScopeHoistingFinalizerContext;

/// Finalizer for emitting output code with scope hoisting.
pub struct ScopeHoistingFinalizer<'me, 'ast> {
  pub ctx: ScopeHoistingFinalizerContext<'me>,
  pub snippet: AstSnippet<'ast>,
  pub ast_scope: &'me AstScopes,
  pub allocator: &'ast Allocator,
  pub namespace_alias_symbol_id: FxHashSet<SymbolId>,
}

impl<'me, 'ast> ScopeHoistingFinalizer<'me, 'ast> {
  pub fn canonical_name_for(&self, symbol: SymbolRef) -> &'me str {
    self.ctx.symbol_ref_db.canonical_name_for(symbol, self.ctx.canonical_names)
  }

  pub fn finalized_expr_for_runtime_symbol(&self, name: &str) -> ast::Expression<'ast> {
    self.finalized_expr_for_symbol_ref(self.ctx.runtime.resolve_symbol(name), false)
  }

  fn finalized_expr_for_symbol_ref(
    &self,
    symbol_ref: SymbolRef,
    preserve_this_semantic_if_needed: bool,
  ) -> ast::Expression<'ast> {
    if !symbol_ref.is_declared_in_root_scope(self.ctx.symbol_ref_db) {
      return self.snippet.id_ref_expr(self.canonical_name_for(symbol_ref), SPAN);
    }

    let mut canonical_ref = self.ctx.symbol_ref_db.canonical_ref_for(symbol_ref);
    let mut canonical_symbol = self.ctx.symbol_ref_db.get(canonical_ref);
    let namespace_alias = &canonical_symbol.namespace_alias;
    if let Some(ns_alias) = namespace_alias {
      canonical_ref = ns_alias.namespace_ref;
      canonical_symbol = self.ctx.symbol_ref_db.get(canonical_ref);
    }

    let mut expr = if self.ctx.modules[canonical_ref.owner].is_external() {
      self.snippet.id_ref_expr(self.canonical_name_for(canonical_ref), SPAN)
    } else {
      match self.ctx.options.format {
        OutputFormat::Cjs => {
          let cur_chunk_idx = self.ctx.chunk_graph.module_to_chunk[self.ctx.id].unwrap();
          let chunk_idx_of_canonical_symbol = canonical_symbol.chunk_id.unwrap_or_else(|| {
            // Scoped symbols don't get assigned a `ChunkId`. There are skipped for performance reason, because they are surely
            // belong to the chunk they are declared in and won't link to other chunks.
            let symbol_name = canonical_ref.name(self.ctx.symbol_ref_db);
            panic!("{canonical_ref:?} {symbol_name:?} is not in any chunk, which is unexpected");
          });
          if cur_chunk_idx != chunk_idx_of_canonical_symbol {
            // In cjs output, we need convert the `import { foo } from 'foo'; console.log(foo);`;
            // If `foo` is split into another chunk, we need to convert the code `console.log(foo);` to `console.log(require_xxxx.foo);`
            // instead of keeping `console.log(foo)` as we did in esm output. The reason here is we need to keep live binding in cjs output.
            self.snippet.literal_prop_access_member_expr_expr(
              &self.ctx.chunk_graph.chunk_table[cur_chunk_idx]
                .require_binding_names_for_other_chunks[&chunk_idx_of_canonical_symbol],
              &self.ctx.chunk_graph.chunk_table[chunk_idx_of_canonical_symbol]
                .exports_to_other_chunks[&canonical_ref],
            )
          } else {
            self.snippet.id_ref_expr(self.canonical_name_for(canonical_ref), SPAN)
          }
        }
        _ => self.snippet.id_ref_expr(self.canonical_name_for(canonical_ref), SPAN),
      }
    };

    if let Some(ns_alias) = namespace_alias {
      expr = ast::Expression::StaticMemberExpression(
        self.snippet.builder.alloc_static_member_expression(
          SPAN,
          expr,
          self.snippet.id_name(&ns_alias.property_name, SPAN),
          false,
        ),
      );

      if preserve_this_semantic_if_needed {
        expr = self.snippet.seq2_in_paren_expr(self.snippet.number_expr(0.0, "0"), expr);
      }
    }

    expr
  }

  fn generate_declaration_of_module_namespace_object(&self) -> Vec<ast::Statement<'ast>> {
    let binding_name_for_namespace_object_ref =
      self.canonical_name_for(self.ctx.module.namespace_object_ref);
    // construct `var [binding_name_for_namespace_object_ref] = {}`
    let decl_stmt = self.snippet.var_decl_stmt(
      binding_name_for_namespace_object_ref,
      ast::Expression::ObjectExpression(ArenaBox::new_in(
        ast::ObjectExpression::dummy(self.allocator),
        self.allocator,
      )),
    );

    let exports_len = self.ctx.linking_info.canonical_exports().count();

    let export_all_externals_rec_ids = &self.ctx.linking_info.star_exports_from_external_modules;

    let mut re_export_external_stmts = None;
    if !export_all_externals_rec_ids.is_empty() {
      // construct `__reExport(importer_exports, importee_exports)`
      let re_export_fn_ref = self.finalized_expr_for_runtime_symbol("__reExport");
      match self.ctx.options.format {
        OutputFormat::Esm => {
          let stmts = export_all_externals_rec_ids.iter().copied().flat_map(|idx| {
            let rec = &self.ctx.module.import_records[idx];
            let importee_namespace_name = self.canonical_name_for(rec.namespace_ref);
            let Some(Module::External(module)) = self.ctx.modules.get(rec.state) else {
              return vec![];
            };
            vec![
              // Insert `import * as ns from 'ext'`external module in esm format
              self.snippet.import_star_stmt(&module.name, importee_namespace_name),
              // Insert `__reExport(foo_exports, ns)`
              self.snippet.builder.statement_expression(
                SPAN,
                self.snippet.call_expr_with_2arg_expr(
                  re_export_fn_ref.clone_in(self.allocator),
                  self.snippet.id_ref_expr(binding_name_for_namespace_object_ref, SPAN),
                  self.snippet.id_ref_expr(importee_namespace_name, SPAN),
                ),
              ),
            ]
          });
          re_export_external_stmts = Some(stmts.collect::<Vec<_>>());
        }
        OutputFormat::Cjs => {
          let stmts = export_all_externals_rec_ids.iter().copied().map(|idx| {
            // Insert `__reExport(importer_exports, require('ext'))`
            let re_export_fn_ref = self.finalized_expr_for_runtime_symbol("__reExport");
            // importer_exports
            let importer_namespace_ref_expr =
              self.finalized_expr_for_symbol_ref(self.ctx.module.namespace_object_ref, false);
            let rec = &self.ctx.module.import_records[idx];
            let importee = &self.ctx.modules[rec.state];
            let expression = self.snippet.call_expr_with_2arg_expr(
              re_export_fn_ref,
              importer_namespace_ref_expr,
              self.snippet.call_expr_with_arg_expr_expr(
                "require",
                self.snippet.string_literal_expr(importee.id(), SPAN),
              ),
            );
            ast::Statement::ExpressionStatement(
              ast::ExpressionStatement { span: expression.span(), expression }
                .into_in(self.allocator),
            )
          });
          re_export_external_stmts = Some(stmts.collect());
        }
      }
    };

    if exports_len == 0 {
      let mut ret = vec![decl_stmt];
      ret.extend(re_export_external_stmts.unwrap_or_default());
      return ret;
    }

    // construct `{ prop_name: () => returned, ... }`
    let mut arg_obj_expr = ast::ObjectExpression::dummy(self.allocator);
    arg_obj_expr.properties.reserve_exact(exports_len);

    self.ctx.linking_info.canonical_exports().for_each(|(export, resolved_export)| {
      // prop_name: () => returned
      let prop_name = export;
      let returned = self.finalized_expr_for_symbol_ref(resolved_export, false);
      arg_obj_expr.properties.push(ast::ObjectPropertyKind::ObjectProperty(
        ast::ObjectProperty {
          key: if is_validate_identifier_name(prop_name) {
            ast::PropertyKey::StaticIdentifier(
              self.snippet.id_name(prop_name, SPAN).into_in(self.allocator),
            )
          } else {
            ast::PropertyKey::StringLiteral(self.snippet.alloc_string_literal(prop_name, SPAN))
          },
          value: self.snippet.only_return_arrow_expr(returned),
          ..ast::ObjectProperty::dummy(self.allocator)
        }
        .into_in(self.allocator),
      ));
    });

    // construct `__export(ns_name, { prop_name: () => returned, ... })`
    let export_call_expr = self.snippet.builder.expression_call(
      SPAN,
      self.finalized_expr_for_runtime_symbol("__export"),
      NONE,
      self.snippet.builder.vec_from_array([
        ast::Argument::from(self.snippet.id_ref_expr(binding_name_for_namespace_object_ref, SPAN)),
        ast::Argument::ObjectExpression(arg_obj_expr.into_in(self.allocator)),
      ]),
      false,
    );
    let export_call_stmt = self.snippet.builder.statement_expression(SPAN, export_call_expr);
    let mut ret = vec![decl_stmt, export_call_stmt];
    ret.extend(re_export_external_stmts.unwrap_or_default());

    ret
  }

  // Handle `import.meta.xxx` expression
  pub fn try_rewrite_import_meta_prop_expr(
    &self,
    mem_expr: &ast::StaticMemberExpression<'ast>,
  ) -> Option<Expression<'ast>> {
    if mem_expr.object.is_import_meta() {
      let original_expr_span = mem_expr.span;
      let is_node_cjs = matches!(
        (self.ctx.options.platform, &self.ctx.options.format),
        (Platform::Node, OutputFormat::Cjs)
      );

      let property_name = mem_expr.property.name.as_str();
      match property_name {
        // Try to polyfill `import.meta.url`
        "url" => {
          let new_expr = if is_node_cjs {
            // Replace it with `require('url').pathToFileURL(__filename).href`

            // require('url')
            let require_call = self.snippet.builder.alloc_call_expression(
              SPAN,
              self.snippet.builder.expression_identifier(SPAN, "require"),
              oxc::ast::NONE,
              self.snippet.builder.vec1(ast::Argument::StringLiteral(
                self.snippet.builder.alloc_string_literal(SPAN, "url", None),
              )),
              false,
            );

            // require('url').pathToFileURL
            let require_path_to_file_url = self.snippet.builder.alloc_static_member_expression(
              SPAN,
              ast::Expression::CallExpression(require_call),
              self.snippet.builder.identifier_name(SPAN, "pathToFileURL"),
              false,
            );

            // require('url').pathToFileURL(__filename)
            let require_path_to_file_url_call = self.snippet.builder.alloc_call_expression(
              SPAN,
              ast::Expression::StaticMemberExpression(require_path_to_file_url),
              oxc::ast::NONE,
              self.snippet.builder.vec1(ast::Argument::Identifier(
                self.snippet.builder.alloc_identifier_reference(SPAN, "__filename"),
              )),
              false,
            );

            // require('url').pathToFileURL(__filename).href
            let require_path_to_file_url_href =
              self.snippet.builder.alloc_static_member_expression(
                original_expr_span,
                ast::Expression::CallExpression(require_path_to_file_url_call),
                self.snippet.builder.identifier_name(SPAN, "href"),
                false,
              );
            Some(ast::Expression::StaticMemberExpression(require_path_to_file_url_href))
          } else {
            None
          };
          return new_expr;
        }
        "dirname" | "filename" => {
          let name = self.snippet.atom(&format!("__{property_name}"));
          return is_node_cjs.then_some(ast::Expression::Identifier(
            self.snippet.builder.alloc_identifier_reference(SPAN, name),
          ));
        }
        _ => {}
      }
    }
    None
  }

  /// try rewrite `foo_exports.bar` or `foo_exports['bar']`  to `bar` directly
  /// try rewrite `import.meta`
  fn try_rewrite_member_expr(
    &mut self,
    member_expr: &ast::MemberExpression<'ast>,
  ) -> Option<Expression<'ast>> {
    match member_expr {
      MemberExpression::StaticMemberExpression(mem_expr) => self
        .ctx
        .linking_info
        .resolved_member_expr_refs
        .get(&mem_expr.span)
        .map(|(object_ref, props)| match object_ref {
          Some(object_ref) => self.snippet.member_expr_or_ident_ref(
            self.finalized_expr_for_symbol_ref(*object_ref, false),
            props,
            mem_expr.span,
          ),
          None => self.snippet.member_expr_with_void_zero_object(props, mem_expr.span),
        })
        .or_else(|| self.try_rewrite_import_meta_prop_expr(mem_expr)),
      MemberExpression::ComputedMemberExpression(inner_expr) => {
        self.ctx.linking_info.resolved_member_expr_refs.get(&inner_expr.span).map(
          |(object_ref, props)| match object_ref {
            Some(object_ref) => self.snippet.member_expr_or_ident_ref(
              self.finalized_expr_for_symbol_ref(*object_ref, false),
              props,
              inner_expr.span,
            ),
            None => self.snippet.member_expr_with_void_zero_object(props, inner_expr.span),
          },
        )
      }
      MemberExpression::PrivateFieldExpression(_) => None,
    }
  }

  fn try_rewrite_inline_dynamic_import_expr(
    &mut self,
    import_expr: &mut ImportExpression<'ast>,
  ) -> Option<Expression<'ast>> {
    if matches!(self.ctx.options.format, OutputFormat::Cjs) {
      // Convert `import('./foo.mjs')` to `Promise.resolve().then(function() { return require('./foo.mjs') })`
      let rec_id = self.ctx.module.imports.get(&import_expr.span)?;
      let rec = &self.ctx.module.import_records[*rec_id];
      let importee_id = rec.state;
      if self.ctx.modules[importee_id].is_normal() {
        let importer_chunk = &self.ctx.chunk_graph.chunk_table[self.ctx.chunk_id];
        let importee_chunk_id = self.ctx.chunk_graph.entry_module_to_chunk[&importee_id];
        let importee_chunk = &self.ctx.chunk_graph.chunk_table[importee_chunk_id];
        let import_path = importer_chunk.import_path_for(importee_chunk);
        return Some(self.snippet.promise_resolve_then_call_expr(
          import_expr.span,
          self.snippet.builder.vec1(ast::Statement::ReturnStatement(
            self.snippet.builder.alloc_return_statement(
              SPAN,
              Some(ast::Expression::CallExpression(self.snippet.builder.alloc_call_expression(
                SPAN,
                self.snippet.builder.expression_identifier(SPAN, "require"),
                NONE,
                self.snippet.builder.vec1(ast::Argument::StringLiteral(
                  self.snippet.alloc_string_literal(&import_path, import_expr.span),
                )),
                false,
              ))),
            ),
          )),
        ));
      }
    }
    None
  }

  fn remove_unused_top_level_stmt(&mut self, program: &mut ast::Program<'ast>) {
    let old_body = program.body.take_in(self.allocator);
    old_body.into_iter().zip(self.ctx.module.stmt_infos.iter().skip(1)).for_each(
      |(mut top_stmt, stmt_info)| {
        if !stmt_info.is_included
          || top_stmt.is_import_declaration()
          || top_stmt.is_export_all_declaration()
        {
          return;
        }

        if let Some(default_decl) = top_stmt.as_export_default_declaration_mut() {
          match &mut default_decl.declaration {
            decl @ ast::match_expression!(ExportDefaultDeclarationKind) => {
              // "export default foo;" => "var default = foo;"
              top_stmt = self.snippet.var_decl_stmt(
                self.canonical_name_for(self.ctx.module.default_export_ref),
                decl.to_expression_mut().take_in(self.allocator),
              );
            }
            ast::ExportDefaultDeclarationKind::FunctionDeclaration(func) => {
              // "export default function() {}" => "function default() {}"
              // "export default function foo() {}" => "function foo() {}"
              if func.id.is_none() {
                func.id = Some(
                  self
                    .snippet
                    .id(self.canonical_name_for(self.ctx.module.default_export_ref), SPAN),
                );
              }
              top_stmt = ast::Statement::FunctionDeclaration(ArenaBox::new_in(
                func.as_mut().take_in(self.allocator),
                self.allocator,
              ));
            }
            ast::ExportDefaultDeclarationKind::ClassDeclaration(class) => {
              // "export default class {}" => "class default {}"
              // "export default class Foo {}" => "class Foo {}"
              if class.id.is_none() {
                class.id = Some(
                  self
                    .snippet
                    .id(self.canonical_name_for(self.ctx.module.default_export_ref), SPAN),
                );
              }
              top_stmt = ast::Statement::ClassDeclaration(ArenaBox::new_in(
                class.as_mut().take_in(self.allocator),
                self.allocator,
              ));
            }
            _ => {}
          }
        } else if let Some(named_decl) = top_stmt.as_export_named_declaration_mut() {
          if named_decl.source.is_some() {
            return;
          }
          let Some(decl) = &mut named_decl.declaration else {
            return;
          };
          // `export var foo = 1` => `var foo = 1`
          // `export function foo() {}` => `function foo() {}`
          // `export class Foo {}` => `class Foo {}`
          top_stmt = ast::Statement::from(decl.take_in(self.allocator));
        }
        program.body.push(top_stmt);
      },
    );
  }
}
