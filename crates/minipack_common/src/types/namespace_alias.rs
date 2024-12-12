use minipack_utils::rstr::Rstr;

use crate::SymbolRef;

#[derive(Debug)]
pub struct NamespaceAlias {
  pub property_name: Rstr,
  pub namespace_ref: SymbolRef,
}
