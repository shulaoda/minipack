use minipack_common::{
  GetLocalDb, ModuleIdx, OutputFormat, SymbolRef, SymbolRefDb, SymbolRefDbForModule,
};
use minipack_utils::concat_string;
use minipack_utils::rstr::{Rstr, ToRstr};
use oxc::syntax::keyword::{GLOBAL_OBJECTS, RESERVED_KEYWORDS};
use rustc_hash::{FxHashMap, FxHashSet};
use std::collections::hash_map::Entry;

/// Manages symbol renaming across modules to prevent name collisions.
/// It tracks used names and generates unique names for symbols.
#[derive(Debug)]
pub struct Renamer<'name> {
  // Maps canonical base names to the next available suffix index (e.g., `a` -> 0 means try `a`, then `a$1`).
  // Used to generate unique mangled names based on original names.
  pub used_canonical_names: FxHashMap<Rstr, u32>,

  // Maps internal SymbolRefs to their final, deconflicted names.
  pub canonical_names: FxHashMap<SymbolRef, Rstr>,

  /// Reference to the shared symbol database for looking up symbol information.
  pub symbol_db: &'name SymbolRefDb,

  // Optional reference to the symbol database specific to the primary entry module.
  pub entry_module: Option<&'name SymbolRefDbForModule>,

  /// Stores names already used within each module's original source code to avoid collisions when renaming.
  pub module_used_names: FxHashMap<ModuleIdx, FxHashSet<&'name str>>,

  /// Tracks all final generated names used globally to ensure uniqueness across the bundle.
  pub used_names: FxHashSet<Rstr>,
}

impl<'name> Renamer<'name> {
  pub fn new(
    base_module_index: Option<ModuleIdx>,
    symbol_db: &'name SymbolRefDb,
    format: OutputFormat,
  ) -> Self {
    let manual_reserved = if matches!(format, OutputFormat::Cjs) {
      vec!["module", "require", "__filename", "__dirname", "exports"]
    } else {
      vec![]
    };

    let used_canonical_names = manual_reserved
      .iter()
      .chain(["Object", "Promise"].iter())
      .chain(RESERVED_KEYWORDS.iter())
      .chain(GLOBAL_OBJECTS.iter())
      .map(|s| (Rstr::new(s), 0))
      .collect();

    let module_used_names = base_module_index
      .map(|index| {
        // Special for entry module, the whole symbol names are stored; other modules only store non-root symbol names.
        FxHashMap::from_iter([(
          index,
          symbol_db.local_db(index).ast_scopes.symbol_names().collect::<FxHashSet<_>>(),
        )])
      })
      .unwrap_or_default();

    Self {
      used_canonical_names,
      canonical_names: FxHashMap::default(),
      symbol_db,
      module_used_names,
      entry_module: base_module_index.map(|index| symbol_db.local_db(index)),
      used_names: FxHashSet::default(),
    }
  }

  pub fn reserve(&mut self, name: Rstr) {
    self.used_canonical_names.insert(name, 0);
  }

  pub fn add_symbol_in_root_scope(&mut self, symbol_ref: SymbolRef) {
    let canonical_ref = symbol_ref.canonical_ref(self.symbol_db);
    let original_name = canonical_ref.name(self.symbol_db);

    self.canonical_names.entry(canonical_ref).or_insert_with(|| {
      let (mut candidate_name, count) =
        match self.used_canonical_names.entry(original_name.to_rstr()) {
          Entry::Occupied(o) => {
            let count = o.into_mut();
            *count += 1;
            (Self::generate_candidate_name(original_name, *count), count)
          }
          Entry::Vacant(v) => (original_name.to_rstr(), v.insert(0)),
        };

      loop {
        let is_root_binding = self.entry_module.is_some_and(|module| {
          module.ast_scopes.get_root_binding(&candidate_name).is_some_and(|symbol_id| {
            let base_symbol = SymbolRef::from((module.owner, symbol_id));
            base_symbol == symbol_ref || base_symbol.canonical_ref(self.symbol_db) == symbol_ref
          })
        });

        if is_root_binding {
          return candidate_name;
        }

        if !self.used_names.contains(&candidate_name)
          // Cannot rename to a name that is already used in the entry module
          && !self.entry_module.is_some_and(|entry|
                self.module_used_names.get(&entry.owner).is_some_and(|used_names| {
                  used_names.contains(candidate_name.as_str())}))
          // Cannot rename to a name that is already used in symbol itself module
          && !self
            .module_used_names
            .entry(symbol_ref.owner)
            .or_insert_with(|| Self::get_module_used_names(self.symbol_db, symbol_ref))
            .contains(candidate_name.as_str())
        {
          self.used_names.insert(candidate_name.clone());
          return candidate_name;
        }

        *count += 1;
        candidate_name = Self::generate_candidate_name(original_name, *count);
      }
    });
  }

  fn generate_candidate_name(original_name: &str, count: u32) -> Rstr {
    concat_string!(original_name, "$", itoa::Buffer::new().format(count)).into()
  }

  fn get_module_used_names(
    symbol_db: &'name SymbolRefDb,
    canonical_ref: SymbolRef,
  ) -> FxHashSet<&'name str> {
    const RUNTIME_MODULE_INDEX: ModuleIdx = ModuleIdx::from_usize_unchecked(0);
    if canonical_ref.owner == RUNTIME_MODULE_INDEX {
      FxHashSet::default()
    } else {
      let scoping = &symbol_db.local_db(canonical_ref.owner).ast_scopes;
      if scoping.symbols_len() == 0 {
        return FxHashSet::default();
      }
      let root_symbol_ids =
        scoping.get_bindings(scoping.root_scope_id()).values().collect::<FxHashSet<_>>();
      scoping
        .symbol_ids()
        .zip(scoping.symbol_names())
        .filter(|(symbol_id, _)| !root_symbol_ids.contains(symbol_id))
        .map(|(_, name)| name)
        .collect::<FxHashSet<&str>>()
    }
  }

  pub fn create_conflictless_name(&mut self, hint: &str) -> String {
    let hint = Rstr::new(hint);
    let mut conflictless_name = hint.clone();
    loop {
      match self.used_canonical_names.entry(conflictless_name.clone()) {
        Entry::Occupied(mut occ) => {
          let next_conflict_index = *occ.get() + 1;
          *occ.get_mut() = next_conflict_index;
          conflictless_name =
            concat_string!(hint, "$", itoa::Buffer::new().format(next_conflict_index)).into();
        }
        Entry::Vacant(vac) => {
          vac.insert(0);
          break;
        }
      }
    }
    conflictless_name.to_string()
  }
}
