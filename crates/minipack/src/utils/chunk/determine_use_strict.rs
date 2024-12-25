use crate::types::generator::GenerateContext;

pub fn determine_use_strict(ctx: &GenerateContext) -> bool {
  let mut modules = ctx.renderable_ecma_modules().peekable();

  if modules.peek().is_none() {
    // No modules, no need to add "use strict"
    return false;
  }

  modules.all(|ecma_module| {
    ecma_module.exports_kind.is_esm()
      || ctx.link_output.index_ecma_ast[ecma_module.ecma_ast_idx()].0.contains_use_strict
  })
}
