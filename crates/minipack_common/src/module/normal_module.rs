use std::fmt::Debug;
use std::sync::Arc;

use crate::ecmascript::ecma_view::{EcmaView, EsmNamespaceInCjs};
use crate::types::interop::Interop;
use crate::{
  AssetView, CssView, ExportsKind, ImportRecordIdx, ImportRecordMeta, ModuleId, ModuleIdx,
  RuntimeModuleBrief, StmtInfo, SymbolRef, SymbolRefDb,
};
use crate::{EcmaAstIdx, Module, ModuleType};
use std::ops::{Deref, DerefMut};

use minipack_ecmascript::{EcmaAst, EcmaCompiler};
use minipack_utils::concat_string;
use minipack_utils::ecmascript::legitimize_identifier_name;
use minipack_utils::rstr::Rstr;
use oxc_index::IndexVec;
use rustc_hash::FxHashSet;

#[derive(Debug)]
pub struct NormalModule {
  pub exec_order: u32,
  pub idx: ModuleIdx,
  pub is_user_defined_entry: bool,
  pub id: ModuleId,
  /// `stable_id` is calculated based on `id` to be stable across machine and os.
  pub stable_id: String,
  // Pretty resource id for debug
  pub debug_id: String,
  pub repr_name: String,
  pub module_type: ModuleType,
  pub ecma_view: EcmaView,
  pub css_view: Option<CssView>,
  pub asset_view: Option<AssetView>,
}

impl NormalModule {
  pub fn star_export_module_ids(&self) -> impl Iterator<Item = ModuleIdx> + '_ {
    if self.has_star_export() {
      itertools::Either::Left(
        self
          .ecma_view
          .import_records
          .iter()
          .filter(|&rec| rec.meta.contains(ImportRecordMeta::IS_EXPORT_STAR))
          .map(|rec| rec.resolved_module),
      )
    } else {
      itertools::Either::Right(std::iter::empty())
    }
  }

  pub fn has_star_export(&self) -> bool {
    self.ecma_view.meta.has_star_export()
  }

  pub fn is_js_type(&self) -> bool {
    matches!(self.module_type, ModuleType::Js | ModuleType::Jsx | ModuleType::Ts | ModuleType::Tsx)
  }

  // https://tc39.es/ecma262/#sec-getexportednames
  pub fn get_exported_names<'modules>(
    &'modules self,
    export_star_set: &mut FxHashSet<ModuleIdx>,
    modules: &'modules IndexVec<ModuleIdx, Module>,
    include_default: bool,
    ret: &mut FxHashSet<&'modules Rstr>,
  ) {
    if export_star_set.contains(&self.idx) {
      return;
    }

    export_star_set.insert(self.idx);

    self
      .star_export_module_ids()
      .filter_map(|id| modules[id].as_normal())
      .for_each(|module| module.get_exported_names(export_star_set, modules, false, ret));
    if include_default {
      ret.extend(self.ecma_view.named_exports.keys());
    } else {
      ret.extend(self.ecma_view.named_exports.keys().filter(|name| name.as_str() != "default"));
    }
  }

  pub fn ecma_ast_idx(&self) -> EcmaAstIdx {
    self.ecma_view.ecma_ast_idx.expect("ecma_ast_idx should be set in this stage")
  }

  pub fn star_exports_from_external_modules<'me>(
    &'me self,
    modules: &'me IndexVec<ModuleIdx, Module>,
  ) -> impl Iterator<Item = ImportRecordIdx> + 'me {
    self.ecma_view.import_records.iter_enumerated().filter_map(move |(rec_id, rec)| {
      if !rec.meta.contains(ImportRecordMeta::IS_EXPORT_STAR) {
        return None;
      }
      match modules[rec.resolved_module] {
        Module::External(_) => Some(rec_id),
        Module::Normal(_) => None,
      }
    })
  }

  // If the module is an ESM module that follows the Node.js ESM spec, such as
  // - extension is `.mjs`
  // - `package.json` has `"type": "module"`
  // , we need to consider to stimulate the Node.js ESM behavior for maximum compatibility.
  pub fn interop(&self, importee: &NormalModule) -> Option<Interop> {
    if matches!(importee.ecma_view.exports_kind, ExportsKind::CommonJs) {
      if self.ecma_view.def_format.is_esm() { Some(Interop::Node) } else { Some(Interop::Babel) }
    } else {
      None
    }
  }

  // If the module is an ESM module that follows the Node.js ESM spec, such as
  // - extension is `.mjs`
  // - `package.json` has `"type": "module"`
  // , we need to consider to stimulate the Node.js ESM behavior for maximum compatibility.
  #[inline]
  pub fn should_consider_node_esm_spec(&self) -> bool {
    self.ecma_view.def_format.is_esm()
  }

  pub fn render(&self, args: &ModuleRenderArgs) -> Option<String> {
    match args {
      ModuleRenderArgs::Ecma { ast } => {
        let render_output = EcmaCompiler::print(ast);
        if !self.ecma_view.mutations.is_empty() {
          let original_code: Arc<str> = render_output.code.into();
          let magic_string = string_wizard::MagicString::new(&*original_code);
          let code = magic_string.to_string();
          return Some(code);
        }
        Some(render_output.code)
      }
    }
  }

  pub fn is_included(&self) -> bool {
    self.ecma_view.meta.is_included()
  }

  /// Generates
  /// ```js
  /// var import_xxx = __toESM(require_xxx());
  /// ```
  pub fn generate_esm_namespace_in_cjs_stmt(
    &mut self,
    symbol_db: &mut SymbolRefDb,
    runtime_module: &RuntimeModuleBrief,
    wrap_ref: SymbolRef,
  ) {
    if self.esm_namespace_in_cjs.is_some() {
      return;
    }
    let esm_namespace_ref_derived_from_module_exports = symbol_db.create_facade_root_symbol_ref(
      self.idx,
      &concat_string!("import_", legitimize_identifier_name(&self.repr_name)),
    );

    let stmt_info_idx = self.stmt_infos.add_stmt_info(StmtInfo {
      stmt_idx: None,
      declared_symbols: vec![esm_namespace_ref_derived_from_module_exports],
      referenced_symbols: vec![wrap_ref.into(), runtime_module.resolve_symbol("__toESM").into()],
      force_tree_shaking: true,
      #[cfg(debug_assertions)]
      debug_label: Some("esm_namespace_ref_derived_from_module_exports".to_string()),
      ..Default::default()
    });

    self.esm_namespace_in_cjs = Some(EsmNamespaceInCjs {
      namespace_ref: esm_namespace_ref_derived_from_module_exports,
      stmt_info_idx,
    });
  }

  /// Generates
  /// ```js
  /// var import_xxx = __toESM(require_xxx(), 1);
  /// ```
  pub fn generate_esm_namespace_in_cjs_node_mode_stmt(
    &mut self,
    symbol_db: &mut SymbolRefDb,
    runtime_module: &RuntimeModuleBrief,
    wrap_ref: SymbolRef,
  ) {
    if self.esm_namespace_in_cjs_node_mode.is_some() {
      return;
    }
    let esm_namespace_ref_derived_from_module_exports = symbol_db.create_facade_root_symbol_ref(
      self.idx,
      &concat_string!("import_", legitimize_identifier_name(&self.repr_name)),
    );

    let stmt_info_idx = self.stmt_infos.add_stmt_info(StmtInfo {
      stmt_idx: None,
      declared_symbols: vec![esm_namespace_ref_derived_from_module_exports],
      referenced_symbols: vec![wrap_ref.into(), runtime_module.resolve_symbol("__toESM").into()],
      force_tree_shaking: true,
      #[cfg(debug_assertions)]
      debug_label: Some("esm_namespace_ref_derived_from_module_exports node".to_string()),
      ..Default::default()
    });

    self.esm_namespace_in_cjs_node_mode = Some(EsmNamespaceInCjs {
      namespace_ref: esm_namespace_ref_derived_from_module_exports,
      stmt_info_idx,
    });
  }
}

impl Deref for NormalModule {
  type Target = EcmaView;

  fn deref(&self) -> &Self::Target {
    &self.ecma_view
  }
}

impl DerefMut for NormalModule {
  fn deref_mut(&mut self) -> &mut Self::Target {
    &mut self.ecma_view
  }
}

pub enum ModuleRenderArgs<'any> {
  Ecma { ast: &'any EcmaAst },
}
