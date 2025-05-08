use minipack_common::SourceJoiner;
use minipack_error::BuildResult;
use minipack_utils::concat_string;

use crate::{
  generate_stage::generators::ecmascript::{RenderedModuleSource, RenderedModuleSources},
  types::generator::GenerateContext,
  utils::chunk::render_chunk_exports::render_chunk_exports,
};

#[inline]
fn render_modules_with_peek_runtime_module_at_first<'a>(
  ctx: &GenerateContext<'_>,
  source_joiner: &mut SourceJoiner<'a>,
  module_sources: &'a RenderedModuleSources,
  import_code: String,
) {
  let mut module_sources_peekable = module_sources.iter().peekable();
  match module_sources_peekable.peek() {
    Some(RenderedModuleSource { module_idx, sources: Some(emitted_sources) })
      if *module_idx == ctx.link_output.runtime_module.id() =>
    {
      for source in emitted_sources.iter() {
        source_joiner.append_source(source);
      }
      module_sources_peekable.next();
    }
    _ => {}
  }

  source_joiner.append_source(import_code);

  module_sources_peekable.for_each(|RenderedModuleSource { sources, .. }| {
    if let Some(emitted_sources) = sources {
      for source in emitted_sources.as_ref() {
        source_joiner.append_source(source);
      }
    }
  });
}

pub fn render_cjs<'code>(
  ctx: &GenerateContext<'_>,
  module_sources: &'code RenderedModuleSources,
) -> BuildResult<SourceJoiner<'code>> {
  let mut source_joiner = SourceJoiner::default();
  let mut modules = ctx.renderable_ecma_modules().peekable();

  if modules.peek().is_some() {
    source_joiner.append_source("\"use strict\";");
  }

  // Runtime module should be placed before the generated `requires` in CJS format.
  // Because, we might need to generate `__toESM(require(...))` that relies on the runtime module.
  render_modules_with_peek_runtime_module_at_first(
    ctx,
    &mut source_joiner,
    module_sources,
    render_cjs_chunk_imports(ctx),
  );

  if let Some(exports) = render_chunk_exports(ctx) {
    source_joiner.append_source(exports);
  }

  Ok(source_joiner)
}

// Make sure the imports generate stmts keep live bindings.
fn render_cjs_chunk_imports(ctx: &GenerateContext<'_>) -> String {
  let mut s = String::new();

  // render imports from other chunks
  ctx.chunk.imports_from_other_chunks.iter().for_each(|(exporter_id, items)| {
    let importee_chunk = &ctx.chunk_graph.chunk_table[*exporter_id];
    let require_path_str =
      concat_string!("require('", ctx.chunk.import_path_for(importee_chunk), "');\n");
    if items.is_empty() {
      s.push_str(&require_path_str);
    } else {
      s.push_str("const ");
      s.push_str(&ctx.chunk.require_binding_names_for_other_chunks[exporter_id]);
      s.push_str(" = ");
      s.push_str(&require_path_str);
    }
  });

  // render external imports
  ctx.chunk.imports_from_external_modules.iter().for_each(|(importee_id, _)| {
    let importee = ctx.link_output.module_table[*importee_id]
      .as_external()
      .expect("Should be external module here");

    let require_path_str = concat_string!("require(\"", &importee.name, "\")");

    if ctx.link_output.used_symbol_refs.contains(&importee.namespace_ref) {
      let to_esm_fn_name = ctx.finalized_string_pattern_for_symbol_ref(
        ctx.link_output.runtime_module.resolve_symbol("__toESM"),
        ctx.chunk_idx,
        &ctx.chunk.canonical_names,
      );

      let external_module_symbol_name = &ctx.chunk.canonical_names[&importee.namespace_ref];
      s.push_str("const ");
      s.push_str(external_module_symbol_name);
      s.push_str(" = ");
      s.push_str(&to_esm_fn_name);
      s.push('(');
      s.push_str(&require_path_str);
      s.push_str(");\n");
    } else if importee.side_effects.has_side_effects() {
      s.push_str(&require_path_str);
      s.push_str(";\n");
    }
  });

  s
}
