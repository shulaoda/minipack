use std::borrow::Cow;

use minipack_common::{Chunk, ChunkKind, ModuleIdx, OutputFormat, SymbolRef, SymbolRefDb};
use minipack_utils::{
  concat_string,
  ecmascript::{property_access_str, to_module_import_export_name},
  indexmap::FxIndexSet,
  rstr::Rstr,
};

use crate::{link_stage::LinkStageOutput, types::generator::GenerateContext};

pub fn render_chunk_exports(ctx: &GenerateContext<'_>) -> Option<String> {
  let GenerateContext { chunk, link_output, options, .. } = ctx;
  let export_items = get_export_items(chunk, link_output).into_iter().collect::<Vec<_>>();

  match options.format {
    OutputFormat::Esm => {
      if export_items.is_empty() {
        return None;
      }
      let mut s = String::new();
      let rendered_items = export_items
        .into_iter()
        .map(|(exported_name, export_ref)| {
          let canonical_ref = link_output.symbols.canonical_ref_for(export_ref);
          let symbol = link_output.symbols.get(canonical_ref);
          let canonical_name = &chunk.canonical_names[&canonical_ref];
          if let Some(ns_alias) = &symbol.namespace_alias {
            let canonical_ns_name = &chunk.canonical_names[&ns_alias.namespace_ref];
            let property_name = &ns_alias.property_name;
            s.push_str(&concat_string!(
              "var ",
              canonical_name,
              " = ",
              canonical_ns_name,
              ".",
              property_name,
              ";\n"
            ));
          }

          if canonical_name == &exported_name {
            Cow::Borrowed(canonical_name.as_str())
          } else {
            Cow::Owned(concat_string!(
              canonical_name,
              " as ",
              to_module_import_export_name(&exported_name)
            ))
          }
        })
        .collect::<Vec<_>>();
      s.push_str(&concat_string!("export { ", rendered_items.join(", "), " };"));
      Some(s)
    }
    OutputFormat::Cjs => {
      let mut s = String::new();
      match chunk.kind {
        ChunkKind::EntryPoint { module, .. } => {
          let module =
            &link_output.module_table[module].as_normal().expect("should be normal module");

          let rendered_items = export_items
            .into_iter()
            .map(|(exported_name, export_ref)| {
              let canonical_ref = link_output.symbols.canonical_ref_for(export_ref);
              let symbol = link_output.symbols.get(canonical_ref);
              let mut canonical_name = Cow::Borrowed(&chunk.canonical_names[&canonical_ref]);
              let exported_value = if let Some(ns_alias) = &symbol.namespace_alias {
                let canonical_ns_name = &chunk.canonical_names[&ns_alias.namespace_ref];
                let property_name = &ns_alias.property_name;
                Cow::Owned(property_access_str(canonical_ns_name, property_name).into())
              } else if link_output.module_table[canonical_ref.owner].is_external() {
                let namespace = &chunk.canonical_names[&canonical_ref];
                Cow::Owned(namespace.as_str().into())
              } else {
                let cur_chunk_idx = ctx.chunk_idx;
                let canonical_ref_owner_chunk_idx =
                  link_output.symbols.get(canonical_ref).chunk_id.unwrap();
                let is_this_symbol_point_to_other_chunk =
                  cur_chunk_idx != canonical_ref_owner_chunk_idx;
                if is_this_symbol_point_to_other_chunk {
                  let require_binding = &ctx.chunk.require_binding_names_for_other_chunks
                    [&canonical_ref_owner_chunk_idx];
                  canonical_name = Cow::Owned(Rstr::new(&concat_string!(
                    require_binding,
                    ".",
                    canonical_name.as_str()
                  )));
                };
                canonical_name.clone()
              };

              if must_keep_live_binding(export_ref, &link_output.symbols) {
                render_object_define_property(&exported_name, &exported_value)
              } else {
                concat_string!(
                  property_access_str("exports", exported_name.as_str()),
                  " = ",
                  exported_value.as_str()
                )
              }
            })
            .collect::<Vec<_>>();
          s.push_str(&rendered_items.join("\n"));

          let meta = &ctx.link_output.metadata[module.idx];
          let external_modules = meta
            .star_exports_from_external_modules
            .iter()
            .map(|rec_idx| module.ecma_view.import_records[*rec_idx].state)
            .collect::<FxIndexSet<ModuleIdx>>();
          external_modules.iter().for_each(|idx| {
          let external = &ctx.link_output.module_table[*idx].as_external().expect("Should be external module here");
          let binding_ref_name =
          &ctx.chunk.canonical_names[&external.namespace_ref];
            let import_stmt =
"Object.keys($NAME).forEach(function (k) {
  if (k !== 'default' && !Object.prototype.hasOwnProperty.call(exports, k)) Object.defineProperty(exports, k, {
    enumerable: true,
    get: function () { return $NAME[k]; }
  });
});\n".replace("$NAME", binding_ref_name);

          s.push_str(&format!("\nvar {} = require(\"{}\");\n", binding_ref_name, &external.name));
          s.push_str(&import_stmt);
        });
        }
        ChunkKind::Common => {
          export_items.into_iter().for_each(|(exported_name, export_ref)| {
            let canonical_ref = link_output.symbols.canonical_ref_for(export_ref);
            let symbol = link_output.symbols.get(canonical_ref);
            let canonical_name = &chunk.canonical_names[&canonical_ref];

            if let Some(ns_alias) = &symbol.namespace_alias {
              let canonical_ns_name = &chunk.canonical_names[&ns_alias.namespace_ref];
              let property_name = &ns_alias.property_name;
              s.push_str(&render_object_define_property(
                &exported_name,
                &concat_string!(canonical_ns_name, ".", property_name),
              ));
            } else {
              s.push_str(&render_object_define_property(&exported_name, canonical_name));
            };
          });
        }
      }

      if s.is_empty() {
        return None;
      }
      Some(s)
    }
  }
}

#[inline]
pub fn render_object_define_property(key: &str, value: &str) -> String {
  concat_string!(
    "Object.defineProperty(exports, '",
    key,
    "', {
  enumerable: true,
  get: function () {
    return ",
    value,
    ";
  }
});"
  )
}

pub fn get_export_items(chunk: &Chunk, graph: &LinkStageOutput) -> Vec<(Rstr, SymbolRef)> {
  match chunk.kind {
    ChunkKind::EntryPoint { module, .. } => {
      let meta = &graph.metadata[module];
      meta
        .canonical_exports()
        .map(|(name, export)| (name.clone(), export.symbol_ref))
        .collect::<Vec<_>>()
    }
    ChunkKind::Common => {
      let mut tmp = chunk
        .exports_to_other_chunks
        .iter()
        .map(|(export_ref, alias)| (alias.clone(), *export_ref))
        .collect::<Vec<_>>();

      tmp.sort_unstable_by(|a, b| a.0.as_str().cmp(b.0.as_str()));

      tmp
    }
  }
}

fn must_keep_live_binding(export_ref: SymbolRef, symbol_db: &SymbolRefDb) -> bool {
  let canonical_ref = symbol_db.canonical_ref_for(export_ref);

  if canonical_ref.is_declared_by_const(symbol_db).unwrap_or(false) {
    // For unknown case, we consider it as not declared by `const`.
    return false;
  }

  if canonical_ref.is_not_reassigned(symbol_db).unwrap_or(false) {
    // For unknown case, we consider it as reassigned.
    return false;
  }

  true
}
