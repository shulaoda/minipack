use oxc::{semantic::SymbolId, span::CompactStr};
use rustc_hash::FxHashMap;

use crate::{AstScopes, ModuleIdx, SymbolRef};

pub static RUNTIME_MODULE_ID: &str = "minipack:runtime";

#[derive(Debug)]
pub struct RuntimeModuleBrief {
  pub idx: ModuleIdx,
  name_to_symbol: FxHashMap<CompactStr, SymbolId>,
}

impl RuntimeModuleBrief {
  pub fn new(idx: ModuleIdx, scope: &AstScopes) -> Self {
    let name_to_symbol = scope
      .get_bindings(scope.root_scope_id())
      .into_iter()
      .map(|(name, &symbol_id)| (CompactStr::new(name), symbol_id))
      .collect();

    Self { idx, name_to_symbol }
  }

  pub fn resolve_symbol(&self, name: &str) -> SymbolRef {
    SymbolRef::from((self.idx, self.name_to_symbol[name]))
  }
}
