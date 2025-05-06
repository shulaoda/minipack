pub mod program_cell;

use std::fmt::Debug;

use arcstr::ArcStr;
use oxc::{allocator::Allocator, ast::ast::Program, span::SourceType};

use self::program_cell::ProgramCell;
use crate::EcmaCompiler;

/// - To access `&mut ast::Program`, use `ast.program.with_mut(|fields| { fields.program; })`.
pub struct EcmaAst {
  pub program: ProgramCell,
  pub source_type: SourceType,
}

impl EcmaAst {
  pub fn source(&self) -> &ArcStr {
    &self.program.borrow_owner().source
  }

  pub fn allocator(&self) -> &Allocator {
    &self.program.borrow_owner().allocator
  }

  pub fn program(&self) -> &Program {
    &self.program.borrow_dependent().program
  }
}

impl Debug for EcmaAst {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("Ast").field("source", &self.source()).finish_non_exhaustive()
  }
}

impl Default for EcmaAst {
  fn default() -> Self {
    EcmaCompiler::parse("", SourceType::default()).unwrap()
  }
}

unsafe impl Send for EcmaAst {}
unsafe impl Sync for EcmaAst {}
