use crate::SymbolRef;

use super::member_expr_ref::MemberExprRef;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum SymbolOrMemberExprRef {
  Symbol(SymbolRef),
  MemberExpr(MemberExprRef),
}

impl SymbolOrMemberExprRef {
  pub fn symbol_ref(&self) -> &SymbolRef {
    match self {
      Self::Symbol(s) => s,
      Self::MemberExpr(expr) => &expr.object_ref,
    }
  }
}

impl From<MemberExprRef> for SymbolOrMemberExprRef {
  fn from(value: MemberExprRef) -> Self {
    Self::MemberExpr(value)
  }
}

impl From<SymbolRef> for SymbolOrMemberExprRef {
  fn from(value: SymbolRef) -> Self {
    Self::Symbol(value)
  }
}
