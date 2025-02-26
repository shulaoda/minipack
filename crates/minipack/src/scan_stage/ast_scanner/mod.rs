mod cjs_ast_analyzer;
mod dynamic_import;
mod impl_visit;
mod import_assign_analyzer;
pub mod pre_processor;
mod side_effect_detector;

use std::borrow::Cow;

use arcstr::ArcStr;
use minipack_common::{
  AstScopes, EcmaModuleAstUsage, ExportsKind, ImportKind, ImportRecordIdx, ImportRecordMeta,
  LocalExport, MemberExprRef, ModuleDefFormat, ModuleId, ModuleIdx, NamedImport, RawImportRecord,
  Specifier, StmtInfo, StmtInfos, SymbolRef, SymbolRefDbForModule, SymbolRefFlags,
  ThisExprReplaceKind,
  dynamic_import_usage::{DynamicImportExportsUsage, DynamicImportUsageInfo},
};
use minipack_ecmascript_utils::{BindingIdentifierExt, BindingPatternExt};
use minipack_error::BuildResult;
use minipack_utils::{
  concat_string, ecmascript::legitimize_identifier_name, path_ext::PathExt, rstr::Rstr,
};
use oxc::{
  ast::{
    AstKind, Comment, Visit,
    ast::{
      self, ExportAllDeclaration, ExportDefaultDeclaration, ExportNamedDeclaration,
      IdentifierReference, ImportDeclaration, MemberExpression, ModuleDeclaration, Program,
    },
  },
  semantic::{Reference, ScopeFlags, ScopeId, ScopeTree, SymbolId, SymbolTable},
  span::{CompactStr, GetSpan, SPAN, Span},
};
use oxc_index::IndexVec;
use rustc_hash::{FxHashMap, FxHashSet};
use sugar_path::SugarPath;

use crate::types::SharedOptions;

#[derive(Debug)]
pub struct AstScanResult {
  pub named_imports: FxHashMap<SymbolRef, NamedImport>,
  pub named_exports: FxHashMap<Rstr, LocalExport>,
  pub stmt_infos: StmtInfos,
  pub import_records: IndexVec<ImportRecordIdx, RawImportRecord>,
  pub default_export_ref: SymbolRef,
  /// Represents [Module Namespace Object](https://tc39.es/ecma262/#sec-module-namespace-exotic-objects)
  pub namespace_object_ref: SymbolRef,
  pub imports: FxHashMap<Span, ImportRecordIdx>,
  pub exports_kind: ExportsKind,
  pub warnings: Vec<anyhow::Error>,
  pub errors: Vec<anyhow::Error>,
  pub has_eval: bool,
  pub ast_usage: EcmaModuleAstUsage,
  pub scopes: AstScopes,
  pub symbols: SymbolRefDbForModule,
  /// https://github.com/evanw/esbuild/blob/d34e79e2a998c21bb71d57b92b0017ca11756912/internal/js_parser/js_parser_lower_class.go#L2277-L2283
  /// used for check if current class decl symbol was referenced in its class scope
  /// We needs to record the info in ast scanner since after that the ast maybe touched, etc
  /// (naming deconflict)
  pub self_referenced_class_decl_symbol_ids: FxHashSet<SymbolId>,
  /// Hashbang only works if it's literally the first character. So we need to generate it in chunk
  /// level rather than module level, or a syntax error will be raised if there are multi modules
  /// has hashbang. Storing the span of hashbang used for hashbang codegen in chunk level
  pub hashbang_range: Option<Span>,
  pub has_star_exports: bool,
  /// We don't know the ImportRecord related ModuleIdx yet, so use ImportRecordIdx as key temporarily
  pub dynamic_import_rec_exports_usage: FxHashMap<ImportRecordIdx, DynamicImportExportsUsage>,
  /// `new URL('...', import.meta.url)`
  pub new_url_references: FxHashMap<Span, ImportRecordIdx>,
  pub this_expr_replace_map: FxHashMap<Span, ThisExprReplaceKind>,
}

pub struct AstScanner<'me, 'ast> {
  idx: ModuleIdx,
  source: &'me ArcStr,
  module_type: ModuleDefFormat,
  id: &'me ModuleId,
  comments: &'me oxc::allocator::Vec<'me, Comment>,
  current_stmt_info: StmtInfo,
  result: AstScanResult,
  esm_export_keyword: Option<Span>,
  esm_import_keyword: Option<Span>,
  /// Cjs ident span used for emit `commonjs_variable_in_esm` warning
  cjs_module_ident: Option<Span>,
  cjs_exports_ident: Option<Span>,
  /// Whether the module is a commonjs module
  /// The reason why we can't reuse `cjs_exports_ident` and `cjs_module_ident` is that
  /// any `module` or `exports` in the top-level scope should be treated as a commonjs module.
  /// `cjs_exports_ident` and `cjs_module_ident` only only recorded when they are appear in
  /// lhs of AssignmentExpression
  ast_usage: EcmaModuleAstUsage,
  cur_class_decl: Option<SymbolId>,
  visit_path: Vec<AstKind<'ast>>,
  scope_stack: Vec<Option<ScopeId>>,
  options: &'me SharedOptions,
  dynamic_import_usage_info: DynamicImportUsageInfo,
  /// "top level" `this` AstNode range in source code
  top_level_this_expr_set: FxHashSet<Span>,
  /// A flag to resolve `this` appear with propertyKey in class
  is_nested_this_inside_class: bool,
}

impl<'me, 'ast: 'me> AstScanner<'me, 'ast> {
  #[allow(clippy::too_many_arguments)]
  pub fn new(
    idx: ModuleIdx,
    scopes: ScopeTree,
    symbols: SymbolTable,
    repr_name: &'me str,
    module_type: ModuleDefFormat,
    source: &'me ArcStr,
    file_path: &'me ModuleId,
    comments: &'me oxc::allocator::Vec<'me, Comment>,
    options: &'me SharedOptions,
  ) -> Self {
    let scopes = AstScopes::new(scopes);
    let mut symbols = SymbolRefDbForModule::new(idx, symbols, scopes.root_scope_id());
    // This is used for converting "export default foo;" => "var default_symbol = foo;"
    let legitimized_repr_name = legitimize_identifier_name(repr_name);
    let default_export_ref =
      symbols.create_facade_root_symbol_ref(&concat_string!(legitimized_repr_name, "_default"));
    // This is used for converting "export default foo;" => "var [default_export_ref] = foo;"
    // And we consider [default_export_ref] never get reassigned.
    default_export_ref.flags_mut(&mut symbols).insert(SymbolRefFlags::IS_NOT_REASSIGNED);

    let name = concat_string!(legitimized_repr_name, "_exports");
    let namespace_object_ref = symbols.create_facade_root_symbol_ref(&name);

    let result = AstScanResult {
      named_imports: FxHashMap::default(),
      named_exports: FxHashMap::default(),
      stmt_infos: {
        let mut stmt_infos = StmtInfos::default();
        // The first `StmtInfo` is used to represent the statement that declares and constructs Module Namespace Object
        stmt_infos.push(StmtInfo::default());
        stmt_infos
      },
      import_records: IndexVec::new(),
      default_export_ref,
      imports: FxHashMap::default(),
      exports_kind: ExportsKind::None,
      warnings: Vec::new(),
      has_eval: false,
      errors: Vec::new(),
      ast_usage: EcmaModuleAstUsage::empty(),
      scopes,
      symbols,
      namespace_object_ref,
      self_referenced_class_decl_symbol_ids: FxHashSet::default(),
      hashbang_range: None,
      has_star_exports: false,
      dynamic_import_rec_exports_usage: FxHashMap::default(),
      new_url_references: FxHashMap::default(),
      this_expr_replace_map: FxHashMap::default(),
    };

    Self {
      idx,
      current_stmt_info: StmtInfo::default(),
      result,
      esm_export_keyword: None,
      esm_import_keyword: None,
      module_type,
      cjs_module_ident: None,
      cjs_exports_ident: None,
      source,
      id: file_path,
      comments,
      ast_usage: EcmaModuleAstUsage::empty()
        .union(EcmaModuleAstUsage::AllStaticExportPropertyAccess),
      cur_class_decl: None,
      visit_path: vec![],
      options,
      scope_stack: vec![],
      dynamic_import_usage_info: DynamicImportUsageInfo::default(),
      top_level_this_expr_set: FxHashSet::default(),
      is_nested_this_inside_class: false,
    }
  }

  /// if current visit path is top level
  pub fn is_valid_tla_scope(&self) -> bool {
    self.scope_stack.iter().rev().filter_map(|item| *item).all(|scope| {
      let flag = self.result.scopes.get_flags(scope);
      flag.is_block() || flag.is_top()
    })
  }

  pub fn scan(mut self, program: &Program<'ast>) -> BuildResult<AstScanResult> {
    self.visit_program(program);
    let mut exports_kind = ExportsKind::None;

    if self.esm_export_keyword.is_some() {
      exports_kind = ExportsKind::Esm;
      if self.cjs_module_ident.is_some() {
        self.result.warnings.push(
          anyhow::anyhow!("The CommonJS module variable is treated as a global variable in an ECMAScript module and may not work as expected")
        );
      }
      if self.cjs_exports_ident.is_some() {
        self.result.warnings.push(
          anyhow::anyhow!("The CommonJS exports variable is treated as a global variable in an ECMAScript module and may not work as expected")
        );
      }
    } else if self.ast_usage.intersects(EcmaModuleAstUsage::ModuleOrExports) {
      exports_kind = ExportsKind::CommonJs;
    } else {
      match self.module_type {
        ModuleDefFormat::CJS | ModuleDefFormat::CjsPackageJson | ModuleDefFormat::Cts => {
          exports_kind = ExportsKind::CommonJs;
        }
        ModuleDefFormat::EsmMjs | ModuleDefFormat::EsmPackageJson | ModuleDefFormat::EsmMts => {
          exports_kind = ExportsKind::Esm;
        }
        ModuleDefFormat::Unknown => {
          if self.esm_import_keyword.is_some() {
            exports_kind = ExportsKind::Esm;
          }
        }
      }
    }

    self.result.ast_usage = self.ast_usage;
    self.result.exports_kind = exports_kind;

    Ok(self.result)
  }

  fn set_esm_export_keyword(&mut self, span: Span) {
    self.esm_export_keyword.get_or_insert(span);
  }

  fn add_declared_id(&mut self, id: SymbolId) {
    self.current_stmt_info.declared_symbols.push((self.idx, id).into());
  }

  fn get_root_binding(&self, name: &str) -> Option<SymbolId> {
    self.result.scopes.get_root_binding(name)
  }

  /// `is_dummy` means if it the import record is created during ast transformation.
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
    let namespace_ref: SymbolRef =
      self.result.symbols.create_facade_root_symbol_ref(&concat_string!(
        "#LOCAL_NAMESPACE_IN_",
        itoa::Buffer::new().format(self.current_stmt_info.stmt_idx.unwrap_or_default().raw()),
        "#"
      ));
    let rec = RawImportRecord::new(
      Rstr::from(module_request),
      kind,
      namespace_ref,
      span,
      None,
      // The first index stmt is reserved for the facade statement that constructs Module Namespace
      // Object
      self.current_stmt_info.stmt_idx.map(|idx| idx + 1),
    )
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
    span_imported: Span,
  ) {
    self.result.named_imports.insert(
      (self.idx, local).into(),
      NamedImport {
        imported: Rstr::new(imported).into(),
        imported_as: (self.idx, local).into(),
        span_imported,
        record_id,
      },
    );
  }

  fn add_star_import(&mut self, local: SymbolId, record_id: ImportRecordIdx, span_imported: Span) {
    self.result.named_imports.insert(
      (self.idx, local).into(),
      NamedImport {
        imported: Specifier::Star,
        imported_as: (self.idx, local).into(),
        record_id,
        span_imported,
      },
    );
  }

  fn add_local_export(&mut self, export_name: &str, local: SymbolId, span: Span) {
    let symbol_ref: SymbolRef = (self.idx, local).into();

    let is_const = self.result.symbols.get_flags(local).is_const_variable();

    // If there is any write reference to the local variable, it is reassigned.
    let is_reassigned = self
      .result
      .scopes
      .get_resolved_references(local, &self.result.symbols)
      .any(Reference::is_write);

    let ref_flags = symbol_ref.flags_mut(&mut self.result.symbols);
    if is_const {
      ref_flags.insert(SymbolRefFlags::IS_CONST);
    }
    if !is_reassigned {
      ref_flags.insert(SymbolRefFlags::IS_NOT_REASSIGNED);
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
    imported: &str,
    record_id: ImportRecordIdx,
    span_imported: Span,
  ) {
    // We will pretend `export { [imported] as [export_name] }` to be `import `
    let ident = if export_name == "default" {
      let importee_repr =
        self.result.import_records[record_id].module_request.as_path().representative_file_name();
      let importee_repr = legitimize_identifier_name(&importee_repr);
      Cow::Owned(concat_string!(importee_repr, "_default"))
    } else {
      // the export_name could be a string literal
      legitimize_identifier_name(export_name)
    };
    let generated_imported_as_ref =
      self.result.symbols.create_facade_root_symbol_ref(ident.as_ref());

    self.current_stmt_info.declared_symbols.push(generated_imported_as_ref);
    let name_import = NamedImport {
      imported: imported.into(),
      imported_as: generated_imported_as_ref,
      record_id,
      span_imported,
    };
    if name_import.imported.is_default() {
      self.result.import_records[record_id].meta.insert(ImportRecordMeta::CONTAINS_IMPORT_DEFAULT);
    }
    self.result.named_exports.insert(
      export_name.into(),
      LocalExport { referenced: generated_imported_as_ref, span: name_import.span_imported },
    );
    self.result.named_imports.insert(generated_imported_as_ref, name_import);
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
      span_imported: span_for_export_name,
      imported_as: generated_imported_as_ref,
      record_id,
    };

    self.result.import_records[record_id].meta.insert(ImportRecordMeta::CONTAINS_IMPORT_STAR);
    self.result.named_exports.insert(
      export_name.into(),
      LocalExport { referenced: generated_imported_as_ref, span: name_import.span_imported },
    );
    self.result.named_imports.insert(generated_imported_as_ref, name_import);
  }

  fn scan_export_all_decl(&mut self, decl: &ExportAllDeclaration) {
    let id = self.add_import_record(
      decl.source.value.as_str(),
      ImportKind::Import,
      decl.source.span(),
      if decl.source.span().is_empty() {
        ImportRecordMeta::IS_UNSPANNED_IMPORT
      } else {
        ImportRecordMeta::empty()
      },
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
        if source.span().is_empty() {
          ImportRecordMeta::IS_UNSPANNED_IMPORT
        } else {
          ImportRecordMeta::empty()
        },
      );
      decl.specifiers.iter().for_each(|spec| {
        self.add_re_export(
          spec.exported.name().as_str(),
          spec.local.name().as_str(),
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
    let ref_id = id_ref.reference_id.get().unwrap_or_else(|| {
      panic!(
        "{id_ref:#?} must have reference id in code```\n{}\n```\n",
        self.current_stmt_info.unwrap_debug_label()
      )
    });
    self.result.scopes.symbol_id_for(ref_id, &self.result.symbols)
  }
  fn scan_export_default_decl(&mut self, decl: &ExportDefaultDeclaration) {
    use oxc::ast::ast::ExportDefaultDeclarationKind;
    let local_binding_for_default_export = match &decl.declaration {
      oxc::ast::match_expression!(ExportDefaultDeclarationKind) => None,
      ast::ExportDefaultDeclarationKind::FunctionDeclaration(fn_decl) => fn_decl
        .id
        .as_ref()
        .map(|id| (minipack_ecmascript_utils::BindingIdentifierExt::expect_symbol_id(id), id.span)),
      ast::ExportDefaultDeclarationKind::ClassDeclaration(cls_decl) => cls_decl
        .id
        .as_ref()
        .map(|id| (minipack_ecmascript_utils::BindingIdentifierExt::expect_symbol_id(id), id.span)),
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
      if decl.source.span().is_empty() {
        ImportRecordMeta::IS_UNSPANNED_IMPORT
      } else {
        ImportRecordMeta::empty()
      },
    );
    self.result.imports.insert(decl.span, rec_id);
    // // `import '...'` or `import {} from '...'`
    if decl.specifiers.as_ref().is_none_or(|s| s.is_empty()) {
      self.result.import_records[rec_id].meta.insert(ImportRecordMeta::IS_PLAIN_IMPORT);
    }

    let Some(specifiers) = &decl.specifiers else { return };
    specifiers.iter().for_each(|spec| match spec {
      ast::ImportDeclarationSpecifier::ImportSpecifier(spec) => {
        let sym = spec.local.expect_symbol_id();
        let imported = spec.imported.name();
        self.add_named_import(sym, imported.as_str(), rec_id, spec.imported.span());
        if imported == "default" {
          self.result.import_records[rec_id].meta.insert(ImportRecordMeta::CONTAINS_IMPORT_DEFAULT);
        }
      }
      ast::ImportDeclarationSpecifier::ImportDefaultSpecifier(spec) => {
        self.add_named_import(spec.local.expect_symbol_id(), "default", rec_id, spec.span);
        self.result.import_records[rec_id].meta.insert(ImportRecordMeta::CONTAINS_IMPORT_DEFAULT);
      }
      ast::ImportDeclarationSpecifier::ImportNamespaceSpecifier(spec) => {
        let symbol_id = spec.local.expect_symbol_id();
        self.add_star_import(symbol_id, rec_id, spec.span);
        self.result.import_records[rec_id].meta.insert(ImportRecordMeta::CONTAINS_IMPORT_STAR);
      }
    });
  }

  fn scan_module_decl(&mut self, decl: &ModuleDeclaration<'ast>) {
    match decl {
      ast::ModuleDeclaration::ImportDeclaration(decl) => {
        self.esm_import_keyword.get_or_insert(Span::new(decl.span.start, decl.span.start + 6));
        self.scan_import_decl(decl);
      }
      ast::ModuleDeclaration::ExportAllDeclaration(decl) => {
        self.set_esm_export_keyword(Span::new(decl.span.start, decl.span.start + 6));
        self.scan_export_all_decl(decl);
      }
      ast::ModuleDeclaration::ExportNamedDeclaration(decl) => {
        self.set_esm_export_keyword(Span::new(decl.span.start, decl.span.start + 6));
        self.scan_export_named_decl(decl);
      }
      ast::ModuleDeclaration::ExportDefaultDeclaration(decl) => {
        self.set_esm_export_keyword(Span::new(decl.span.start, decl.span.start + 6));
        self.scan_export_default_decl(decl);
        if let ast::ExportDefaultDeclarationKind::ClassDeclaration(class) = &decl.declaration {
          self.visit_class(class);
          // walk::walk_declaration(self, &ast::Declaration::ClassDeclaration(func));
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
    self.result.scopes.root_scope_id() == self.result.symbols.get_scope_id(symbol_id)
  }

  fn try_diagnostic_forbid_const_assign(&mut self, id_ref: &IdentifierReference) -> Option<()> {
    let ref_id = id_ref.reference_id.get()?;
    let reference = &self.result.symbols.references[ref_id];
    if reference.is_write() {
      let symbol_id = reference.symbol_id()?;
      if self.result.symbols.get_flags(symbol_id).is_const_variable() {
        self.result.errors.push(anyhow::anyhow!(
          "Unexpected re-assignment of const variable `{0}` at {1}",
          self.result.symbols.get_name(symbol_id),
          self.id.to_string()
        ));
      }
    }
    None
  }

  /// return a `Some(SymbolRef)` if the identifier referenced a top level `IdentBinding`
  fn resolve_identifier_reference(
    &mut self,
    ident: &IdentifierReference,
  ) -> IdentifierReferenceKind {
    match self.resolve_symbol_from_reference(ident) {
      Some(symbol_id) => {
        if self.is_root_symbol(symbol_id) {
          IdentifierReferenceKind::Root((self.idx, symbol_id).into())
        } else {
          IdentifierReferenceKind::Other
        }
      }
      None => IdentifierReferenceKind::Global,
    }
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
    (!props.is_empty()).then_some((span, props))
  }

  // `console` in `console.log` is a global reference
  pub fn is_global_identifier_reference(&self, ident: &IdentifierReference) -> bool {
    let symbol_id = self.resolve_symbol_from_reference(ident);
    symbol_id.is_none()
  }

  /// If it is not a top level `this` reference visit position
  pub fn is_this_nested(&self) -> bool {
    self.is_nested_this_inside_class
      || self.scope_stack.iter().any(|scope| {
        scope.map_or(false, |scope| {
          let flags = self.result.scopes.get_flags(scope);
          flags.contains(ScopeFlags::Function) && !flags.contains(ScopeFlags::Arrow)
        })
      })
  }
}

#[derive(Debug, Clone, Copy)]
pub enum IdentifierReferenceKind {
  /// global variable
  Global,
  /// top level variable
  Root(SymbolRef),
  /// rest
  Other,
}
