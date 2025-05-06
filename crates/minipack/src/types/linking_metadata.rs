use minipack_common::{ImportRecordIdx, ModuleIdx, SymbolRef};
use minipack_utils::{indexmap::FxIndexSet, rstr::Rstr};
use oxc::span::{CompactStr, Span};

use rustc_hash::FxHashMap;

/// Module metadata about linking
#[derive(Debug, Default)]
pub struct LinkingMetadata {
  // Store the export info for each module, including export named declaration and export star declaration.
  pub resolved_exports: FxHashMap<Rstr, SymbolRef>,
  // Store the names of exclude ambiguous resolved exports.
  // It will be used to generate chunk exports and module namespace binding.
  pub sorted_resolved_exports: Vec<Rstr>,
  // Entry chunks need to generate code that doesn't belong to any module. This is the list of symbols are referenced by the
  // generated code. Tree-shaking will cares about these symbols to make sure they are not removed.
  pub referenced_symbols_by_entry_point_chunk: Vec<SymbolRef>,
  /// The dependencies of the module. It means if you want include this module, you need to include these dependencies too.
  pub dependencies: FxIndexSet<ModuleIdx>,
  // `None` the member expression resolve to a ambiguous export.
  pub resolved_member_expr_refs: FxHashMap<Span, (Option<SymbolRef>, Vec<CompactStr>)>,
  pub star_exports_from_external_modules: Vec<ImportRecordIdx>,
}

impl LinkingMetadata {
  pub fn canonical_exports(&self) -> impl Iterator<Item = (&Rstr, SymbolRef)> {
    self.sorted_resolved_exports.iter().map(|name| (name, self.resolved_exports[name]))
  }
}
