use minipack_common::{ImportRecordIdx, ModuleIdx, SymbolRef};
use minipack_utils::{indexmap::FxIndexSet, rstr::Rstr};
use oxc::span::{CompactStr, Span};

use rustc_hash::FxHashMap;

/// Metadata generated for a module during the linking phase.
/// It stores information about resolved exports, dependencies, and runtime requirements.
#[derive(Debug, Default)]
pub struct LinkingMetadata {
  /// The direct module dependencies required for this module's execution.
  /// If this module is included, all modules in this set must also be included.
  pub dependencies: FxIndexSet<ModuleIdx>,

  // Maps export names (including those from 'export *') to the SymbolRef they ultimately resolve to.
  pub resolved_exports: FxHashMap<Rstr, SymbolRef>,

  // An ordered list of canonical export names from this module.
  // Used for stable code generation, such as properties on a namespace object.
  // This list typically excludes names found to be ambiguous during linking.
  pub sorted_resolved_exports: Vec<Rstr>,

  // Stores resolutions for static property access expressions (e.g., `Namespace.property`).
  // Key is the Span of the expression. Value is a tuple:
  // - Option<SymbolRef>: Resolved symbol if successful and unambiguous.
  // - Vec<CompactStr>: Chain of property names accessed (e.g., ["prop1", "prop2"]).
  // A None symbol indicates ambiguous or failed resolution.
  pub resolved_member_expr_refs: FxHashMap<Span, (Option<SymbolRef>, Vec<CompactStr>)>,

  // Symbols that are directly referenced by generated chunk boilerplate or runtime code,
  // and must be preserved from tree-shaking.
  pub referenced_symbols_by_entry_point_chunk: Vec<SymbolRef>,

  // Import records corresponding to 'export * from 'external-module'` statements.
  // These external exports are handled differently as their contents are unknown internally.
  pub star_exports_from_external_modules: Vec<ImportRecordIdx>,
}

impl LinkingMetadata {
  /// Provides an iterator over the canonical export names and their resolved symbols,
  /// in the stable order defined by `sorted_resolved_exports`.
  pub fn canonical_exports(&self) -> impl Iterator<Item = (&Rstr, SymbolRef)> {
    self.sorted_resolved_exports.iter().map(|name| (name, self.resolved_exports[name]))
  }
}
