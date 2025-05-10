use minipack_utils::option_ext::OptionExt;
use oxc::semantic::SymbolId;
use oxc_index::Idx;

use crate::{ModuleIdx, SymbolRefDb, SymbolRefFlags};

use super::symbol_ref_db::{GetLocalDb, GetLocalDbMut};

/// `SymbolRef` is used to represent a symbol in a module when there are multiple modules.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SymbolRef {
  pub owner: ModuleIdx,
  pub symbol: SymbolId,
}

impl Default for SymbolRef {
  fn default() -> Self {
    Self { owner: ModuleIdx::from_raw(0), symbol: SymbolId::from_usize(0) }
  }
}

impl From<(ModuleIdx, SymbolId)> for SymbolRef {
  fn from(value: (ModuleIdx, SymbolId)) -> Self {
    Self { owner: value.0, symbol: value.1 }
  }
}

impl SymbolRef {
  pub fn name<'db>(&self, db: &'db SymbolRefDb) -> &'db str {
    db[self.owner].unpack_ref().symbol_name(self.symbol)
  }

  pub fn set_name(&self, db: &mut SymbolRefDb, name: &str) {
    db[self.owner].unpack_ref_mut().set_symbol_name(self.symbol, name);
  }

  /// Not all symbols have flags info, we only care about part of them.
  /// If you want to ensure the flags info exists, use `flags_mut` instead.
  pub fn flags<'db, T: GetLocalDb>(&self, db: &'db T) -> Option<&'db SymbolRefFlags> {
    db.local_db(self.owner).symbol_flags.get(&self.symbol)
  }

  pub fn flags_mut<'db, T: GetLocalDbMut>(&self, db: &'db mut T) -> &'db mut SymbolRefFlags {
    db.local_db_mut(self.owner).symbol_flags.entry(self.symbol).or_default()
  }

  // `None` means we don't know if it's declared by `const`.
  pub fn is_declared_by_const(&self, db: &SymbolRefDb) -> Option<bool> {
    let flags = self.flags(db)?;
    // Not having this flag means we don't know if it's declared by `const` instead of it's not declared by `const`.
    flags.contains(SymbolRefFlags::IS_CONST).then_some(true)
  }

  /// `None` means we don't know if it gets reassigned.
  pub fn is_not_reassigned(&self, db: &SymbolRefDb) -> Option<bool> {
    let flags = self.flags(db)?;
    // Not having this flag means we don't know
    flags.contains(SymbolRefFlags::IS_NOT_REASSIGNED).then_some(true)
  }

  pub fn is_declared_in_root_scope(&self, db: &SymbolRefDb) -> bool {
    db.is_declared_in_root_scope(*self)
  }

  #[must_use]
  pub fn canonical_ref(&self, db: &SymbolRefDb) -> Self {
    db.canonical_ref_for(*self)
  }
}
