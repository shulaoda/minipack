mod impl_visit;
mod pre_processor;
mod side_effect_detector;

pub use pre_processor::PreProcessor;

use std::borrow::Cow;

use minipack_common::{
  ImportKind, ImportRecordIdx, ImportRecordMeta, LocalExport, MemberExprRef, ModuleIdx,
  NamedImport, RawImportRecord, Specifier, StmtInfo, StmtInfos, SymbolRef, SymbolRefDbForModule,
  SymbolRefFlags,
};
use minipack_ecmascript::{BindingIdentifierExt, BindingPatternExt};
use minipack_error::BuildResult;
use minipack_utils::{concat_string, path_ext::PathExt, rstr::Rstr};
use oxc::{
  ast::{
    AstKind,
    ast::{
      self, ExportAllDeclaration, ExportDefaultDeclaration, ExportNamedDeclaration,
      IdentifierReference, ImportDeclaration, MemberExpression, ModuleDeclaration, Program,
    },
  },
  ast_visit::Visit,
  semantic::{Reference, Scoping, SymbolId},
  span::{CompactStr, GetSpan, SPAN, Span},
};
use oxc_index::IndexVec;
use rustc_hash::FxHashMap;
use sugar_path::SugarPath;

use crate::utils::ecmascript::legitimize_identifier_name;

#[derive(Debug)]
pub struct AstScanResult {
  pub symbols: SymbolRefDbForModule,
  pub stmt_infos: StmtInfos,
  pub has_star_exports: bool,
  pub default_export_ref: SymbolRef,
  pub namespace_object_ref: SymbolRef,
  pub imports: FxHashMap<Span, ImportRecordIdx>,
  pub named_imports: FxHashMap<SymbolRef, NamedImport>,
  pub named_exports: FxHashMap<Rstr, LocalExport>,
  pub import_records: IndexVec<ImportRecordIdx, RawImportRecord>,
  pub errors: Vec<anyhow::Error>,
  pub warnings: Vec<anyhow::Error>,
}

pub struct AstScanner<'ast> {
  idx: ModuleIdx,
  result: AstScanResult,
  current_stmt_info: StmtInfo,
  visit_path: Vec<AstKind<'ast>>,
}

impl<'ast> AstScanner<'ast> {
  pub fn new(idx: ModuleIdx, scoping: Scoping, repr_name: &str) -> Self {
    let root_scope_id = scoping.root_scope_id();
    let mut symbol_ref_db = SymbolRefDbForModule::new(idx, scoping, root_scope_id);
    // This is used for converting "export default foo;" => "var default_symbol = foo;"
    let legitimized_repr_name = legitimize_identifier_name(repr_name);
    let default_export_ref = symbol_ref_db
      .create_facade_root_symbol_ref(&concat_string!(legitimized_repr_name, "_default"));

    // This is used for converting "export default foo;" => "var [default_export_ref] = foo;"
    // And we consider [default_export_ref] never get reassigned.
    default_export_ref.flags_mut(&mut symbol_ref_db).insert(SymbolRefFlags::IS_NOT_REASSIGNED);

    let name = concat_string!(legitimized_repr_name, "_exports");
    let namespace_object_ref = symbol_ref_db.create_facade_root_symbol_ref(&name);

    let result = AstScanResult {
      named_imports: FxHashMap::default(),
      named_exports: FxHashMap::default(),
      stmt_infos: {
        // The first `StmtInfo` is used to represent the statement
        // that declares and constructs Module Namespace Object
        let mut stmt_infos = StmtInfos::default();
        stmt_infos.push(StmtInfo::default());
        stmt_infos
      },
      import_records: IndexVec::new(),
      default_export_ref,
      imports: FxHashMap::default(),
      warnings: Vec::new(),
      errors: Vec::new(),
      symbols: symbol_ref_db,
      namespace_object_ref,
      has_star_exports: false,
    };

    Self { idx, current_stmt_info: StmtInfo::default(), result, visit_path: vec![] }
  }

  pub fn scan(mut self, program: &Program<'ast>) -> BuildResult<AstScanResult> {
    self.visit_program(program);
    Ok(self.result)
  }

  fn add_declared_id(&mut self, id: SymbolId) {
    self.current_stmt_info.declared_symbols.push((self.idx, id).into());
  }

  fn get_root_binding(&self, name: &str) -> Option<SymbolId> {
    self.result.symbols.get_root_binding(name)
  }

  fn add_import_record(
    &mut self,
    module_request: &str,
    kind: ImportKind,
    span: Span,
    init_meta: ImportRecordMeta,
  ) -> ImportRecordIdx {
    // If 'foo' in `import ... from 'foo'` is finally a commonjs module, we will convert the import statement
    // to `var import_foo = __toESM(require_foo())`, so we create a symbol for `import_foo` here. Notice that we
    // just create the symbol. If the symbol is finally used would be determined in the linking stage.
    let namespace_ref = self.result.symbols.create_facade_root_symbol_ref(&concat_string!(
      "#LOCAL_NAMESPACE_IN_",
      itoa::Buffer::new().format(self.current_stmt_info.stmt_idx.unwrap_or_default().raw()),
      "#"
    ));

    let rec = RawImportRecord::new(Rstr::from(module_request), kind, namespace_ref, span)
      .with_meta(init_meta);

    let id = self.result.import_records.push(rec);
    self.current_stmt_info.import_records.push(id);
    id
  }

  fn add_named_import(
    &mut self,
    local: SymbolId,
    imported: &str,
    record_id: ImportRecordIdx,
    imported_span: Span,
  ) {
    self.result.named_imports.insert(
      (self.idx, local).into(),
      NamedImport {
        imported: Rstr::new(imported).into(),
        imported_as: (self.idx, local).into(),
        imported_span,
        record_id,
      },
    );
  }

  fn add_star_import(&mut self, local: SymbolId, record_id: ImportRecordIdx, imported_span: Span) {
    self.result.named_imports.insert(
      (self.idx, local).into(),
      NamedImport {
        imported: Specifier::Star,
        imported_as: (self.idx, local).into(),
        record_id,
        imported_span,
      },
    );
  }

  fn add_local_export(&mut self, export_name: &str, local: SymbolId, span: Span) {
    let symbol_ref: SymbolRef = (self.idx, local).into();
    let is_const = self.result.symbols.symbol_flags(local).is_const_variable();
    let is_reassigned = self.result.symbols.get_resolved_references(local).any(Reference::is_write);

    let symbol_ref_flags = symbol_ref.flags_mut(&mut self.result.symbols);

    if is_const {
      symbol_ref_flags.insert(SymbolRefFlags::IS_CONST);
    }
    if !is_reassigned {
      symbol_ref_flags.insert(SymbolRefFlags::IS_NOT_REASSIGNED);
    }

    self
      .result
      .named_exports
      .insert(export_name.into(), LocalExport { referenced: (self.idx, local).into(), span });
  }

  fn add_local_default_export(&mut self, local: SymbolId, span: Span) {
    self
      .result
      .named_exports
      .insert("default".into(), LocalExport { referenced: (self.idx, local).into(), span });
  }

  /// Record `export { [imported] as [export_name] } from ...` statement.
  ///
  /// Notice that we will pretend
  /// ```js
  /// export { [imported] as [export_name] } from '...'
  /// ```
  /// to be
  /// ```js
  /// import { [imported] as [generated] } from '...'
  /// export { [generated] as [export_name] }
  /// ```
  /// Reasons are:
  /// - No extra logic for dealing with re-exports concept.
  /// - Cjs compatibility. We need a [generated] binding to holds the value reexport from commonjs. For example
  /// ```js
  /// export { foo } from 'commonjs'
  /// ```
  /// would be converted to
  /// ```js
  /// const import_commonjs = __toESM(require_commonjs())
  /// const [generated] = import_commonjs.foo
  /// export { [generated] as foo }
  /// ```
  /// `export { foo } from 'commonjs'` would be converted to `const import_commonjs = require()` in the linking stage.
  fn add_re_export(
    &mut self,
    export_name: &str,
    imported: Specifier,
    record_id: ImportRecordIdx,
    imported_span: Span,
  ) {
    // We will pretend `export { [imported] as [export_name] }` to be `import `
    let ident = if export_name == "default" {
      let importee_repr =
        self.result.import_records[record_id].specifier.as_path().representative_file_name();
      let importee_repr = legitimize_identifier_name(&importee_repr);
      Cow::Owned(concat_string!(importee_repr, "_default"))
    } else {
      legitimize_identifier_name(export_name)
    };

    let generated_imported_as_ref =
      self.result.symbols.create_facade_root_symbol_ref(ident.as_ref());

    self.current_stmt_info.declared_symbols.push(generated_imported_as_ref);
    self.result.named_exports.insert(
      export_name.into(),
      LocalExport { referenced: generated_imported_as_ref, span: imported_span },
    );
    self.result.named_imports.insert(
      generated_imported_as_ref,
      NamedImport { imported, imported_as: generated_imported_as_ref, record_id, imported_span },
    );
  }

  fn add_star_re_export(
    &mut self,
    export_name: &str,
    record_id: ImportRecordIdx,
    span_for_export_name: Span,
  ) {
    let generated_imported_as_ref =
      self.result.symbols.create_facade_root_symbol_ref(&legitimize_identifier_name(export_name));
    self.current_stmt_info.declared_symbols.push(generated_imported_as_ref);
    let name_import = NamedImport {
      imported: Specifier::Star,
      imported_span: span_for_export_name,
      imported_as: generated_imported_as_ref,
      record_id,
    };

    self.result.named_exports.insert(
      export_name.into(),
      LocalExport { referenced: generated_imported_as_ref, span: name_import.imported_span },
    );
    self.result.named_imports.insert(generated_imported_as_ref, name_import);
  }

  fn scan_export_all_decl(&mut self, decl: &ExportAllDeclaration) {
    let id = self.add_import_record(
      decl.source.value.as_str(),
      ImportKind::Import,
      decl.source.span(),
      ImportRecordMeta::empty(),
    );
    if let Some(exported) = &decl.exported {
      // export * as ns from '...'
      self.add_star_re_export(exported.name().as_str(), id, decl.span);
    } else {
      // export * from '...'
      self.result.import_records[id].meta.insert(ImportRecordMeta::IS_EXPORT_STAR);
      self.result.has_star_exports = true;
    }
    self.result.imports.insert(decl.span, id);
  }

  fn scan_export_named_decl(&mut self, decl: &ExportNamedDeclaration) {
    if let Some(source) = &decl.source {
      let record_id = self.add_import_record(
        source.value.as_str(),
        ImportKind::Import,
        source.span(),
        ImportRecordMeta::empty(),
      );
      decl.specifiers.iter().for_each(|spec| {
        self.add_re_export(
          spec.exported.name().as_str(),
          spec.local.name().as_str().into(),
          record_id,
          spec.local.span(),
        );
      });
      self.result.imports.insert(decl.span, record_id);
      // `export {} from '...'`
      if decl.specifiers.is_empty() {
        self.result.import_records[record_id].meta.insert(ImportRecordMeta::IS_PLAIN_IMPORT);
      }
    } else {
      decl.specifiers.iter().for_each(|spec| {
        if let Some(local_symbol_id) = self.get_root_binding(spec.local.name().as_str()) {
          self.add_local_export(spec.exported.name().as_str(), local_symbol_id, spec.span);
        } else {
          self
            .result
            .errors
            .push(anyhow::anyhow!("`{}` is not declared in this file", spec.local.name()));
        }
      });
      if let Some(decl) = decl.declaration.as_ref() {
        match decl {
          ast::Declaration::VariableDeclaration(var_decl) => {
            var_decl.declarations.iter().for_each(|decl| {
              decl.id.binding_identifiers().into_iter().for_each(|id| {
                self.add_local_export(&id.name, id.expect_symbol_id(), id.span);
              });
            });
          }
          ast::Declaration::FunctionDeclaration(fn_decl) => {
            let id = fn_decl.id.as_ref().unwrap();
            self.add_local_export(id.name.as_str(), id.expect_symbol_id(), id.span);
          }
          ast::Declaration::ClassDeclaration(cls_decl) => {
            let id = cls_decl.id.as_ref().unwrap();
            self.add_local_export(id.name.as_str(), id.expect_symbol_id(), id.span);
          }
          _ => unreachable!("doesn't support ts now"),
        }
      }
    }
  }

  // If the reference is a global variable, `None` will be returned.
  fn resolve_symbol_from_reference(&self, id_ref: &IdentifierReference) -> Option<SymbolId> {
    let reference_id =
      id_ref.reference_id.get().unwrap_or_else(|| panic!("{id_ref:#?} must have reference id"));
    self.result.symbols.ast_scopes.symbol_id_for(reference_id)
  }

  fn scan_export_default_decl(&mut self, decl: &ExportDefaultDeclaration) {
    use oxc::ast::ast::ExportDefaultDeclarationKind;
    let local_binding_for_default_export = match &decl.declaration {
      oxc::ast::match_expression!(ExportDefaultDeclarationKind) => None,
      ast::ExportDefaultDeclarationKind::FunctionDeclaration(fn_decl) => fn_decl
        .id
        .as_ref()
        .map(|id| (minipack_ecmascript::BindingIdentifierExt::expect_symbol_id(id), id.span)),
      ast::ExportDefaultDeclarationKind::ClassDeclaration(cls_decl) => cls_decl
        .id
        .as_ref()
        .map(|id| (minipack_ecmascript::BindingIdentifierExt::expect_symbol_id(id), id.span)),
      ast::ExportDefaultDeclarationKind::TSInterfaceDeclaration(_) => unreachable!(),
    };

    let (reference, span) = local_binding_for_default_export
      .unwrap_or((self.result.default_export_ref.symbol, Span::default()));

    self.add_declared_id(reference);
    self.add_local_default_export(reference, span);
  }

  fn scan_import_decl(&mut self, decl: &ImportDeclaration) {
    let rec_id = self.add_import_record(
      decl.source.value.as_str(),
      ImportKind::Import,
      decl.source.span(),
      ImportRecordMeta::empty(),
    );
    self.result.imports.insert(decl.span, rec_id);
    // `import '...'` or `import {} from '...'`
    if decl.specifiers.as_ref().is_none_or(|s| s.is_empty()) {
      self.result.import_records[rec_id].meta.insert(ImportRecordMeta::IS_PLAIN_IMPORT);
    }

    let Some(specifiers) = &decl.specifiers else { return };
    specifiers.iter().for_each(|spec| match spec {
      ast::ImportDeclarationSpecifier::ImportSpecifier(spec) => {
        let sym = spec.local.expect_symbol_id();
        let imported = spec.imported.name();
        self.add_named_import(sym, imported.as_str(), rec_id, spec.imported.span());
      }
      ast::ImportDeclarationSpecifier::ImportDefaultSpecifier(spec) => {
        self.add_named_import(spec.local.expect_symbol_id(), "default", rec_id, spec.span);
      }
      ast::ImportDeclarationSpecifier::ImportNamespaceSpecifier(spec) => {
        let symbol_id = spec.local.expect_symbol_id();
        self.add_star_import(symbol_id, rec_id, spec.span);
      }
    });
  }

  fn scan_module_decl(&mut self, decl: &ModuleDeclaration<'ast>) {
    match decl {
      ast::ModuleDeclaration::ImportDeclaration(decl) => {
        self.scan_import_decl(decl);
      }
      ast::ModuleDeclaration::ExportAllDeclaration(decl) => {
        self.scan_export_all_decl(decl);
      }
      ast::ModuleDeclaration::ExportNamedDeclaration(decl) => {
        self.scan_export_named_decl(decl);
      }
      ast::ModuleDeclaration::ExportDefaultDeclaration(decl) => {
        self.scan_export_default_decl(decl);
        if let ast::ExportDefaultDeclarationKind::ClassDeclaration(class) = &decl.declaration {
          self.visit_class(class);
        }
      }
      _ => {}
    }
  }

  pub fn add_referenced_symbol(&mut self, sym_ref: SymbolRef) {
    self.current_stmt_info.referenced_symbols.push(sym_ref.into());
  }

  pub fn add_member_expr_reference(
    &mut self,
    object_ref: SymbolRef,
    props: Vec<CompactStr>,
    span: Span,
  ) {
    self
      .current_stmt_info
      .referenced_symbols
      .push(MemberExprRef::new(object_ref, props, span).into());
  }

  fn is_root_symbol(&self, symbol_id: SymbolId) -> bool {
    self.result.symbols.root_scope_id() == self.result.symbols.symbol_scope_id(symbol_id)
  }

  fn process_identifier_ref_by_scope(&mut self, ident_ref: &IdentifierReference) {
    if let Some(root_symbol_id) = self.resolve_identifier_reference(ident_ref) {
      // if the identifier_reference is a NamedImport MemberExpr access, we store it as a `MemberExpr`
      // use this flag to avoid insert it as `Symbol` at the same time.
      let mut is_inserted_before = false;
      if self.result.named_imports.contains_key(&root_symbol_id) {
        if let Some((span, props)) = self.try_extract_parent_static_member_expr_chain(usize::MAX) {
          is_inserted_before = true;
          self.add_member_expr_reference(root_symbol_id, props, span);
        }
      }

      if !is_inserted_before {
        self.add_referenced_symbol(root_symbol_id);
      }
    }
  }

  /// Return a `Some(SymbolRef)` if the identifier referenced a top level `IdentBinding`
  fn resolve_identifier_reference(&mut self, ident: &IdentifierReference) -> Option<SymbolRef> {
    self
      .resolve_symbol_from_reference(ident)
      .and_then(|symbol_id| self.is_root_symbol(symbol_id).then_some((self.idx, symbol_id).into()))
  }

  /// StaticMemberExpression or ComputeMemberExpression with static key
  pub fn try_extract_parent_static_member_expr_chain(
    &self,
    max_len: usize,
  ) -> Option<(Span, Vec<CompactStr>)> {
    let mut span = SPAN;
    let mut props = vec![];
    for ancestor_ast in self.visit_path.iter().rev().take(max_len) {
      match ancestor_ast {
        AstKind::MemberExpression(MemberExpression::StaticMemberExpression(expr)) => {
          span = ancestor_ast.span();
          props.push(expr.property.name.as_str().into());
        }
        AstKind::MemberExpression(MemberExpression::ComputedMemberExpression(expr)) => {
          if let Some(name) = expr.static_property_name() {
            span = ancestor_ast.span();
            props.push(name.into());
          } else {
            break;
          }
        }
        _ => break,
      }
    }
    (!props.is_empty() && span != SPAN).then_some((span, props))
  }
}
