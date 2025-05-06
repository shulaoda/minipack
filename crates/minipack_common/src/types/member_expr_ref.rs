use oxc::span::{CompactStr, Span};
use rustc_hash::FxHashMap;

use crate::SymbolRef;

/// For member expression, e.g. `foo_ns.bar_ns.c`
/// - `object_ref` is the `SymbolRef` that represents `foo_ns`
/// - `props` is `["bar_ns", "c"]`
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct MemberExprRef {
  pub span: Span,
  pub object_ref: SymbolRef,
  pub props: Vec<CompactStr>,
}

impl MemberExprRef {
  pub fn new(object_ref: SymbolRef, props: Vec<CompactStr>, span: Span) -> Self {
    Self { object_ref, props, span }
  }

  pub fn resolved_symbol_ref(
    &self,
    resolved_map: &FxHashMap<Span, (Option<SymbolRef>, Vec<CompactStr>)>,
  ) -> Option<SymbolRef> {
    if let Some((resolved, _)) = resolved_map.get(&self.span) {
      resolved.as_ref().map(|sym_ref| *sym_ref)
    } else {
      Some(self.object_ref)
    }
  }
}
