mod finalizer_context;
mod impl_visit_mut;
mod rename;

pub use finalizer_context::ScopeHoistingFinalizerContext;

use oxc::{
  allocator::{Allocator, Box as ArenaBox, CloneIn, Dummy, IntoIn, TakeIn},
  ast::{
    NONE,
    ast::{self, ExportDefaultDeclarationKind, Expression, ImportExpression, MemberExpression},
  },
  semantic::{ReferenceId, SymbolId},
  span::{GetSpan, SPAN},
};
use rustc_hash::FxHashSet;

use minipack_common::{AstScopes, Module, OutputFormat, Platform, SymbolRef};
use minipack_ecmascript::{AstSnippet, ExpressionExt, StatementExt};
use minipack_utils::ecmascript::is_validate_identifier_name;

/// Finalizer for emitting output code with scope hoisting.
pub struct ScopeHoistingFinalizer<'me, 'ast> {
  pub ctx: ScopeHoistingFinalizerContext<'me>,
  pub scope: &'me AstScopes,
  pub alloc: &'ast Allocator,
  pub snippet: AstSnippet<'ast>,
  /// `SymbolRef` imported from a cjs module which has `namespace_alias`
  /// more details please refer [`rolldown_common::types::symbol_ref_db::SymbolRefDataClassic`].
  pub namespace_alias_symbol_id: FxHashSet<SymbolId>,
  /// All `ReferenceId` of `IdentifierReference` we are interested, the `IdentifierReference` should be the object of `MemberExpression` and the property is not
  /// a `"default"` property access
  pub interested_namespace_alias_ref_id: FxHashSet<ReferenceId>,
}

impl<'me, 'ast> ScopeHoistingFinalizer<'me, 'ast> {
  pub fn canonical_name_for(&self, symbol: SymbolRef) -> &'me str {
    self.ctx.symbol_ref_db.canonical_name_for(symbol, self.ctx.canonical_names)
  }

  pub fn finalized_expr_for_runtime_symbol(&self, name: &str) -> ast::Expression<'ast> {
    self.finalized_expr_for_symbol_ref(self.ctx.runtime.resolve_symbol(name), false)
  }

  fn try_get_valid_namespace_alias_ref_id_from_member_expr(
    &self,
    member_expr: &MemberExpression<'ast>,
  ) -> Option<ReferenceId> {
    if member_expr.static_property_name()? == "default" {
      return None;
    }

    let ident_ref = match member_expr {
      MemberExpression::ComputedMemberExpression(expr) => expr.object.as_identifier()?,
      MemberExpression::StaticMemberExpression(expr) => expr.object.as_identifier()?,
      MemberExpression::PrivateFieldExpression(_) => return None,
    };

    let reference_id = ident_ref.reference_id.get()?;
    let symbol_id = self.scope.symbol_id_for(reference_id)?;
    if !self.namespace_alias_symbol_id.contains(&symbol_id) {
      return None;
    }

    Some(reference_id)
  }

  fn finalized_expr_for_symbol_ref(
    &self,
    symbol_ref: SymbolRef,
    preserve_this_semantic_if_needed: bool,
  ) -> ast::Expression<'ast> {
    if !symbol_ref.is_declared_in_root_scope(self.ctx.symbol_ref_db) {
      // No fancy things on none root scope symbols
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
          let chunk_idx_of_canonical_symbol =
            canonical_symbol.chunk_id.unwrap_or_else(|| {
              // Scoped symbols don't get assigned a `ChunkId`. There are skipped for performance reason, because they are surely
              // belong to the chunk they are declared in and won't link to other chunks.
              let symbol_name = canonical_ref.name(self.ctx.symbol_ref_db);
              eprintln!(
                "{canonical_ref:?} {symbol_name:?} is not in any chunk, which is unexpected",
              );
              panic!("{canonical_ref:?} {symbol_name:?} is not in any chunk, which is unexpected");
            });
          let cur_chunk_idx = self.ctx.chunk_graph.module_to_chunk[self.ctx.id]
            .expect("This module should be in a chunk");
          let is_symbol_in_other_chunk = cur_chunk_idx != chunk_idx_of_canonical_symbol;
          if is_symbol_in_other_chunk {
            // In cjs output, we need convert the `import { foo } from 'foo'; console.log(foo);`;
            // If `foo` is split into another chunk, we need to convert the code `console.log(foo);` to `console.log(require_xxxx.foo);`
            // instead of keeping `console.log(foo)` as we did in esm output. The reason here is we need to keep live binding in cjs output.
            let exported_name = &self.ctx.chunk_graph.chunk_table[chunk_idx_of_canonical_symbol]
              .exports_to_other_chunks[&canonical_ref];

            let require_binding = &self.ctx.chunk_graph.chunk_table[cur_chunk_idx]
              .require_binding_names_for_other_chunks[&chunk_idx_of_canonical_symbol];

            self.snippet.literal_prop_access_member_expr_expr(require_binding, exported_name)
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
        ast::ObjectExpression::dummy(self.alloc),
        self.alloc,
      )),
    );

    let exports_len = self.ctx.linking_info.canonical_exports().count();

    let export_all_externals_rec_ids = &self.ctx.linking_info.star_exports_from_external_modules;

    let mut re_export_external_stmts: Option<_> = None;
    if !export_all_externals_rec_ids.is_empty() {
      // construct `__reExport(importer_exports, importee_exports)`
      let re_export_fn_ref = self.finalized_expr_for_runtime_symbol("__reExport");
      match self.ctx.options.format {
        OutputFormat::Esm => {
          let stmts = export_all_externals_rec_ids.iter().copied().flat_map(|idx| {
            let rec = &self.ctx.module.import_records[idx];
            // importee_exports
            let importee_namespace_name = self.canonical_name_for(rec.namespace_ref);
            let m = self.ctx.modules.get(rec.state);
            let Some(Module::External(module)) = m else {
              return vec![];
            };
            let importee_name = &module.name;
            vec![
              // Insert `import * as ns from 'ext'`external module in esm format
              self.snippet.import_star_stmt(importee_name, importee_namespace_name),
              // Insert `__reExport(foo_exports, ns)`
              self.snippet.builder.statement_expression(
                SPAN,
                self.snippet.call_expr_with_2arg_expr(
                  re_export_fn_ref.clone_in(self.alloc),
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
              ast::ExpressionStatement { span: expression.span(), expression }.into_in(self.alloc),
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
    let mut arg_obj_expr = ast::ObjectExpression::dummy(self.alloc);
    arg_obj_expr.properties.reserve_exact(exports_len);

    self.ctx.linking_info.canonical_exports().for_each(|(export, resolved_export)| {
      // prop_name: () => returned
      let prop_name = export;
      let returned = self.finalized_expr_for_symbol_ref(resolved_export, false);
      arg_obj_expr.properties.push(ast::ObjectPropertyKind::ObjectProperty(
        ast::ObjectProperty {
          key: if is_validate_identifier_name(prop_name) {
            ast::PropertyKey::StaticIdentifier(
              self.snippet.id_name(prop_name, SPAN).into_in(self.alloc),
            )
          } else {
            ast::PropertyKey::StringLiteral(self.snippet.alloc_string_literal(prop_name, SPAN))
          },
          value: self.snippet.only_return_arrow_expr(returned),
          ..ast::ObjectProperty::dummy(self.alloc)
        }
        .into_in(self.alloc),
      ));
    });

    // construct `__export(ns_name, { prop_name: () => returned, ... })`
    let export_call_expr = self.snippet.builder.expression_call(
      SPAN,
      self.finalized_expr_for_runtime_symbol("__export"),
      NONE,
      self.snippet.builder.vec_from_array([
        ast::Argument::from(self.snippet.id_ref_expr(binding_name_for_namespace_object_ref, SPAN)),
        ast::Argument::ObjectExpression(arg_obj_expr.into_in(self.alloc)),
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
            // If we don't support polyfill `import.meta.url` in this platform and format, we just keep it as it is
            // so users may handle it in their own way.
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
      MemberExpression::ComputedMemberExpression(inner_expr) => {
        if let Some((object_ref, props)) =
          self.ctx.linking_info.resolved_member_expr_refs.get(&inner_expr.span)
        {
          match object_ref {
            Some(object_ref) => {
              let object_ref_expr = self.finalized_expr_for_symbol_ref(*object_ref, false);

              let replaced_expr =
                self.snippet.member_expr_or_ident_ref(object_ref_expr, props, inner_expr.span);
              return Some(replaced_expr);
            }
            None => {
              return Some(self.snippet.member_expr_with_void_zero_object(props, inner_expr.span));
            }
          }
        }
        None
      }
      MemberExpression::StaticMemberExpression(mem_expr) => {
        if let Some((object_ref, props)) =
          self.ctx.linking_info.resolved_member_expr_refs.get(&mem_expr.span)
        {
          match object_ref {
            Some(object_ref) => {
              let object_ref_expr = self.finalized_expr_for_symbol_ref(*object_ref, false);

              let replaced_expr =
                self.snippet.member_expr_or_ident_ref(object_ref_expr, props, mem_expr.span);
              return Some(replaced_expr);
            }
            None => {
              return Some(self.snippet.member_expr_with_void_zero_object(props, mem_expr.span));
            }
          }
          // these two branch are exclusive since `import.meta` is a global member_expr
        } else if let Some(new_expr) = self.try_rewrite_import_meta_prop_expr(mem_expr) {
          return Some(new_expr);
        }
        None
      }
      MemberExpression::PrivateFieldExpression(_) => None,
    }
  }

  fn try_rewrite_inline_dynamic_import_expr(
    &mut self,
    import_expr: &mut ImportExpression<'ast>,
  ) -> Option<Expression<'ast>> {
    if matches!(self.ctx.options.format, OutputFormat::Cjs) {
      // Convert `import('./foo.mjs')` to `Promise.resolve().then(function() { return require('foo.mjs') })`
      let rec_id = self.ctx.module.imports.get(&import_expr.span)?;
      let rec = &self.ctx.module.import_records[*rec_id];
      let importee_id = rec.state;
      match &self.ctx.modules[importee_id] {
        Module::Normal(_) => {
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
        Module::External(_) => {}
      }
    }
    None
  }

  fn remove_unused_top_level_stmt(&mut self, program: &mut ast::Program<'ast>) {
    let old_body = program.body.take_in(self.alloc);

    // the first statement info is the namespace variable declaration
    // skip first statement info to make sure `program.body` has same index as `stmt_infos`
    old_body.into_iter().zip(self.ctx.module.stmt_infos.iter().skip(1)).for_each(
      |(mut top_stmt, stmt_info)| {
        if !stmt_info.is_included {
          return;
        }

        if top_stmt.is_import_declaration() || top_stmt.is_export_all_declaration() {
          return;
        }

        if let Some(default_decl) = top_stmt.as_export_default_declaration_mut() {
          match &mut default_decl.declaration {
            decl @ ast::match_expression!(ExportDefaultDeclarationKind) => {
              let expr = decl.to_expression_mut();
              // "export default foo;" => "var default = foo;"
              let canonical_name_for_default_export_ref =
                self.canonical_name_for(self.ctx.module.default_export_ref);
              top_stmt = self
                .snippet
                .var_decl_stmt(canonical_name_for_default_export_ref, expr.take_in(self.alloc));
            }
            ast::ExportDefaultDeclarationKind::FunctionDeclaration(func) => {
              // "export default function() {}" => "function default() {}"
              // "export default function foo() {}" => "function foo() {}"
              if func.id.is_none() {
                let canonical_name_for_default_export_ref =
                  self.canonical_name_for(self.ctx.module.default_export_ref);
                func.id = Some(self.snippet.id(canonical_name_for_default_export_ref, SPAN));
              }
              top_stmt = ast::Statement::FunctionDeclaration(ArenaBox::new_in(
                func.as_mut().take_in(self.alloc),
                self.alloc,
              ));
            }
            ast::ExportDefaultDeclarationKind::ClassDeclaration(class) => {
              // "export default class {}" => "class default {}"
              // "export default class Foo {}" => "class Foo {}"
              if class.id.is_none() {
                let canonical_name_for_default_export_ref =
                  self.canonical_name_for(self.ctx.module.default_export_ref);
                class.id = Some(self.snippet.id(canonical_name_for_default_export_ref, SPAN));
              }
              top_stmt = ast::Statement::ClassDeclaration(ArenaBox::new_in(
                class.as_mut().take_in(self.alloc),
                self.alloc,
              ));
            }
            _ => {}
          }
        } else if let Some(named_decl) = top_stmt.as_export_named_declaration_mut() {
          if named_decl.source.is_none() {
            if let Some(decl) = &mut named_decl.declaration {
              // `export var foo = 1` => `var foo = 1`
              // `export function foo() {}` => `function foo() {}`
              // `export class Foo {}` => `class Foo {}`
              top_stmt = ast::Statement::from(decl.take_in(self.alloc));
            } else {
              // `export { foo }`
              // Remove this statement by ignoring it
              return;
            }
          } else {
            return;
          }
        }

        program.body.push(top_stmt);
      },
    );
  }
}
