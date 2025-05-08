pub mod program_cell;

use arcstr::ArcStr;
use oxc::{ast::ast::Program, span::SourceType};

use self::program_cell::ProgramCell;

pub struct EcmaAst {
  pub program: ProgramCell,
  pub source_type: SourceType,
}

impl EcmaAst {
  pub fn source(&self) -> &ArcStr {
    &self.program.borrow_owner().source
  }

  pub fn program(&self) -> &Program {
    &self.program.borrow_dependent().program
  }
}

impl std::fmt::Debug for EcmaAst {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("Ast").field("source", &self.source()).finish_non_exhaustive()
  }
}

unsafe impl Send for EcmaAst {}
unsafe impl Sync for EcmaAst {}
