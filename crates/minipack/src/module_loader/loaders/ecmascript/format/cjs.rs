use minipack_common::{ExportsKind, OutputExports, WrapKind};
use minipack_error::BuildResult;
use minipack_sourcemap::SourceJoiner;
use minipack_utils::concat_string;

use crate::{
  module_loader::loaders::ecmascript::ecma_generator::RenderedModuleSources,
  types::generator::GenerateContext,
  utils::chunk::{
    determine_export_mode::determine_export_mode,
    determine_use_strict::determine_use_strict,
    namespace_marker::render_namespace_markers,
    render_chunk_exports::{get_chunk_export_names, render_chunk_exports},
  },
};

fn render_modules_with_peek_runtime_module_at_first<'a>(
  ctx: &GenerateContext<'_>,
  source_joiner: &mut SourceJoiner<'a>,
  module_sources: &'a RenderedModuleSources,
  import_code: String,
) {
  let mut module_sources_peekable = module_sources.iter().peekable();
  match module_sources_peekable.peek() {
    Some((id, _, _)) if *id == ctx.link_output.runtime_brief.id() => {
      if let (_, _module_id, Some(emitted_sources)) =
        module_sources_peekable.next().expect("Must have module")
      {
        for source in emitted_sources.iter() {
          source_joiner.append_source(source);
        }
      }
    }
    _ => {}
  }

  source_joiner.append_source(import_code);

  // chunk content
  module_sources_peekable.for_each(|(_, _, module_render_output)| {
    if let Some(emitted_sources) = module_render_output {
      for source in emitted_sources.as_ref() {
        source_joiner.append_source(source);
      }
    }
  });
}

#[allow(clippy::too_many_lines, clippy::too_many_arguments)]
pub fn render_cjs<'code>(
  ctx: &GenerateContext<'_>,
  module_sources: &'code RenderedModuleSources,
  warnings: &mut Vec<anyhow::Error>,
) -> BuildResult<SourceJoiner<'code>> {
  let mut source_joiner = SourceJoiner::default();

  if determine_use_strict(ctx) {
    source_joiner.append_source("\"use strict\";");
  }

  // Note that the determined `export_mode` should be used in `render_chunk_exports` to render exports.
  // We also need to get the export mode for rendering the namespace markers.
  // So we determine the export mode (from auto) here and use it in the following code.
  let export_mode = if let Some(entry_module) =
    ctx.chunk.user_defined_entry_module(&ctx.link_output.module_table)
  {
    if matches!(entry_module.exports_kind, ExportsKind::Esm) {
      let export_names = get_chunk_export_names(ctx.chunk, ctx.link_output);
      let has_default_export = export_names.iter().any(|name| name.as_str() == "default");
      let export_mode = determine_export_mode(warnings, ctx, entry_module, &export_names)?;
      // Only `named` export can we render the namespace markers.
      if matches!(&export_mode, OutputExports::Named) {
        if let Some(marker) =
          render_namespace_markers(ctx.options.es_module, has_default_export, false)
        {
          source_joiner.append_source(marker.to_string());
        }
      }
      Some(export_mode)
    } else {
      // The entry module which non-ESM export kind should be `named`.
      Some(OutputExports::Named)
    }
  } else {
    // The common chunks should be `named`.
    Some(OutputExports::Named)
  };

  // Runtime module should be placed before the generated `requires` in CJS format.
  // Because, we might need to generate `__toESM(require(...))` that relies on the runtime module.
  render_modules_with_peek_runtime_module_at_first(
    ctx,
    &mut source_joiner,
    module_sources,
    render_cjs_chunk_imports(ctx),
  );

  if let Some(entry_id) = ctx.chunk.entry_module_idx() {
    let entry_meta = &ctx.link_output.metadata[entry_id];
    match entry_meta.wrap_kind {
      WrapKind::Esm => {
        let wrapper_ref = entry_meta.wrapper_ref.as_ref().unwrap();
        // init_xxx
        let wrapper_ref_name = ctx.finalized_string_pattern_for_symbol_ref(
          *wrapper_ref,
          ctx.chunk_idx,
          &ctx.chunk.canonical_names,
        );
        source_joiner.append_source(concat_string!(wrapper_ref_name, "();"));
      }
      WrapKind::Cjs => {
        let wrapper_ref = entry_meta.wrapper_ref.as_ref().unwrap();

        // require_xxx
        let wrapper_ref_name = ctx.finalized_string_pattern_for_symbol_ref(
          *wrapper_ref,
          ctx.chunk_idx,
          &ctx.chunk.canonical_names,
        );

        // module.exports = require_xxx();
        source_joiner.append_source(concat_string!("module.exports = ", wrapper_ref_name, "();\n"));
      }
      WrapKind::None => {}
    }
  }

  if let Some(exports) = render_chunk_exports(ctx, export_mode.as_ref()) {
    source_joiner.append_source(exports);
  }

  Ok(source_joiner)
}

// Make sure the imports generate stmts keep live bindings.
fn render_cjs_chunk_imports(ctx: &GenerateContext<'_>) -> String {
  let mut s = String::new();

  // render imports from other chunks
  ctx.chunk.imports_from_other_chunks.iter().for_each(|(exporter_id, items)| {
    let importee_chunk = &ctx.graph.chunk_table[*exporter_id];
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
    let importee = ctx.link_output.module_table.modules[*importee_id]
      .as_external()
      .expect("Should be external module here");

    let require_path_str = concat_string!("require(\"", &importee.name, "\")");

    if ctx.link_output.used_symbol_refs.contains(&importee.namespace_ref) {
      let to_esm_fn_name = ctx.finalized_string_pattern_for_symbol_ref(
        ctx.link_output.runtime_brief.resolve_symbol("__toESM"),
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
