use std::ops::{Deref, DerefMut, Index, IndexMut};

use minipack_utils::{option_ext::OptionExt, rstr::Rstr};
use oxc::semantic::{NodeId, ScopeId, SymbolFlags};
use oxc::semantic::{Scoping, SymbolId};
use oxc::span::SPAN;
use oxc_index::IndexVec;
use rustc_hash::FxHashMap;

use crate::{AstScopes, ChunkIdx, ModuleIdx, SymbolRef};

use super::namespace_alias::NamespaceAlias;

#[derive(Debug, Default, Clone)]
pub struct SymbolRefDataClassic {
  /// The symbol that this symbol is linked to.
  pub link: Option<SymbolRef>,
  /// The chunk that this symbol is defined in.
  pub chunk_id: Option<ChunkIdx>,
  /// For `import {a} from 'foo.cjs'`, the symbol `a` refers to `module.exports.a`,
  /// it should be rewritten as `foo_ns.a` and `foo_ns` is the namespace of `foo.cjs`.
  /// If `namespace_alias` is set, this symbol must be rewritten as a property access.
  pub namespace_alias: Option<NamespaceAlias>,
}

bitflags::bitflags! {
  #[derive(Debug, Default)]
  pub struct SymbolRefFlags: u8 {
    const IS_CONST = 1;
    const IS_NOT_REASSIGNED = 1 << 1;
  }
}

#[derive(Debug)]
pub struct SymbolRefDbForModule {
  pub owner: ModuleIdx,
  pub ast_scopes: AstScopes,
  pub root_scope_id: ScopeId,
  pub symbol_flags: FxHashMap<SymbolId, SymbolRefFlags>,
  pub classic_data: IndexVec<SymbolId, SymbolRefDataClassic>,
}

impl SymbolRefDbForModule {
  pub fn new(owner: ModuleIdx, scoping: Scoping, root_scope_id: ScopeId) -> Self {
    let classic_data =
      IndexVec::from_vec(vec![SymbolRefDataClassic::default(); scoping.symbols_len()]);

    Self {
      owner,
      ast_scopes: AstScopes::new(scoping),
      classic_data,
      root_scope_id,
      symbol_flags: FxHashMap::default(),
    }
  }

  // The `facade` means the symbol is actually not exist in the AST.
  pub fn create_facade_root_symbol_ref(&mut self, name: &str) -> SymbolRef {
    self.classic_data.push(SymbolRefDataClassic::default());
    SymbolRef::from((
      self.owner,
      self.ast_scopes.create_symbol(
        SPAN,
        name,
        SymbolFlags::empty(),
        self.root_scope_id,
        NodeId::DUMMY,
      ),
    ))
  }
}

impl Deref for SymbolRefDbForModule {
  type Target = Scoping;

  fn deref(&self) -> &Self::Target {
    &self.ast_scopes
  }
}

impl DerefMut for SymbolRefDbForModule {
  fn deref_mut(&mut self) -> &mut Self::Target {
    &mut self.ast_scopes
  }
}

// Information about symbols for all modules
#[derive(Debug, Default)]
pub struct SymbolRefDb {
  inner: IndexVec<ModuleIdx, Option<SymbolRefDbForModule>>,
}

impl Index<ModuleIdx> for SymbolRefDb {
  type Output = Option<SymbolRefDbForModule>;

  fn index(&self, index: ModuleIdx) -> &Self::Output {
    self.inner.index(index)
  }
}

impl IndexMut<ModuleIdx> for SymbolRefDb {
  fn index_mut(&mut self, index: ModuleIdx) -> &mut Self::Output {
    self.inner.index_mut(index)
  }
}

impl SymbolRefDb {
  fn ensure_exact_capacity(&mut self, module_idx: ModuleIdx) {
    let new_len = module_idx.index() + 1;
    if self.inner.len() < new_len {
      self.inner.resize_with(new_len, || None);
    }
  }

  pub fn store_local_db(&mut self, idx: ModuleIdx, local_db: SymbolRefDbForModule) {
    self.ensure_exact_capacity(idx);
    self.inner[idx] = Some(local_db);
  }

  pub fn create_facade_root_symbol_ref(&mut self, owner: ModuleIdx, name: &str) -> SymbolRef {
    self.ensure_exact_capacity(owner);
    self.inner[owner].unpack_ref_mut().create_facade_root_symbol_ref(name)
  }

  /// Make `base` point to `target`
  pub fn link(&mut self, base: SymbolRef, target: SymbolRef) {
    let base_root = self.find_mut(base);
    let target_root = self.find_mut(target);
    if base_root == target_root {
      // already linked
      return;
    }
    self.get_mut(base_root).link = Some(target_root);
  }

  pub fn canonical_name_for<'a>(
    &'a self,
    refer: SymbolRef,
    canonical_names: &'a FxHashMap<SymbolRef, Rstr>,
  ) -> &'a str {
    let canonical_ref = self.canonical_ref_for(refer);
    canonical_names.get(&canonical_ref).map_or_else(move || refer.name(self), Rstr::as_str)
  }

  pub fn get(&self, refer: SymbolRef) -> &SymbolRefDataClassic {
    &self.inner[refer.owner].unpack_ref().classic_data[refer.symbol]
  }

  pub fn get_mut(&mut self, refer: SymbolRef) -> &mut SymbolRefDataClassic {
    &mut self.inner[refer.owner].unpack_ref_mut().classic_data[refer.symbol]
  }

  /// <https://en.wikipedia.org/wiki/Disjoint-set_data_structure>
  /// See Path halving
  pub fn find_mut(&mut self, target: SymbolRef) -> SymbolRef {
    let mut canonical = target;
    while let Some(parent) = self.get_mut(canonical).link {
      self.get_mut(canonical).link = self.get_mut(parent).link;
      canonical = parent;
    }

    canonical
  }

  // Used for the situation where rust require `&self`
  pub fn canonical_ref_for(&self, target: SymbolRef) -> SymbolRef {
    let mut canonical = target;
    while let Some(founded) = self.get(canonical).link {
      debug_assert!(founded != target);
      canonical = founded;
    }
    canonical
  }

  pub fn is_declared_in_root_scope(&self, refer: SymbolRef) -> bool {
    let local_db = self.inner[refer.owner].unpack_ref();
    local_db.symbol_scope_id(refer.symbol) == local_db.root_scope_id
  }
}

pub trait GetLocalDb {
  fn local_db(&self, owner: ModuleIdx) -> &SymbolRefDbForModule;
}

pub trait GetLocalDbMut {
  fn local_db_mut(&mut self, owner: ModuleIdx) -> &mut SymbolRefDbForModule;
}

impl GetLocalDb for SymbolRefDb {
  fn local_db(&self, owner: ModuleIdx) -> &SymbolRefDbForModule {
    self.inner[owner].unpack_ref()
  }
}

impl GetLocalDbMut for SymbolRefDb {
  fn local_db_mut(&mut self, owner: ModuleIdx) -> &mut SymbolRefDbForModule {
    self.inner[owner].unpack_ref_mut()
  }
}

impl GetLocalDb for SymbolRefDbForModule {
  fn local_db(&self, owner: ModuleIdx) -> &SymbolRefDbForModule {
    debug_assert!(self.owner == owner);
    self
  }
}

impl GetLocalDbMut for SymbolRefDbForModule {
  fn local_db_mut(&mut self, owner: ModuleIdx) -> &mut SymbolRefDbForModule {
    debug_assert!(self.owner == owner);
    self
  }
}
