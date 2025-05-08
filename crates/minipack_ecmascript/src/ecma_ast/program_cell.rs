use arcstr::ArcStr;
use oxc::{allocator::Allocator, ast::ast::Program};
use self_cell::self_cell;

pub struct ProgramCellOwner {
  pub source: ArcStr,
  pub allocator: Allocator,
}

pub struct ProgramCellDependent<'cell> {
  pub program: Program<'cell>,
}

self_cell!(
  pub struct ProgramCell {
    owner: ProgramCellOwner,

    #[covariant]
    dependent: ProgramCellDependent,
  }
);

pub struct WithMutFields<'outer, 'inner> {
  pub source: &'inner ArcStr,
  pub allocator: &'inner Allocator,
  pub program: &'outer mut Program<'inner>,
}

impl ProgramCell {
  pub fn with_mut<'outer, Ret>(
    &'outer mut self,
    func: impl for<'inner> ::core::ops::FnOnce(WithMutFields<'outer, 'inner>) -> Ret,
  ) -> Ret {
    self.with_dependent_mut::<'outer, Ret>(
      |owner: &ProgramCellOwner, dependent: &'outer mut ProgramCellDependent| {
        func(WithMutFields {
          source: &owner.source,
          allocator: &owner.allocator,
          program: &mut dependent.program,
        })
      },
    )
  }
}
