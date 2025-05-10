use arcstr::ArcStr;

use minipack_common::{Chunk, ChunkIdx, ChunkKind, GetLocalDb, OutputFormat};
use minipack_utils::rstr::ToRstr;
use rustc_hash::FxHashMap;

use crate::{
  link_stage::LinkStageOutput,
  utils::{ecmascript::legitimize_identifier_name, renamer::Renamer},
};

pub fn deconflict_chunk_symbols(
  chunk: &mut Chunk,
  link_stage_output: &LinkStageOutput,
  format: OutputFormat,
  index_chunk_id_to_name: &FxHashMap<ChunkIdx, ArcStr>,
) {
  let mut renamer =
    Renamer::new(chunk.entry_module_idx(), &link_stage_output.symbol_ref_db, format);

  if matches!(format, OutputFormat::Cjs) {
    chunk.imports_from_external_modules.iter().for_each(|(idx, _)| {
      if let Some(external_module) = link_stage_output.module_table[*idx].as_external() {
        renamer.add_symbol_in_root_scope(external_module.namespace_ref);
      }
    });

    if let Some(module) = chunk.entry_module_idx() {
      let entry_module = link_stage_output.module_table[module].as_normal().unwrap();
      link_stage_output.metadata[entry_module.idx]
        .star_exports_from_external_modules
        .iter()
        .for_each(|rec_idx| {
          let rec = &entry_module.ecma_view.import_records[*rec_idx];
          let external_module = &link_stage_output.module_table[rec.state].as_external().unwrap();
          renamer.add_symbol_in_root_scope(external_module.namespace_ref);
        });
    }
  }

  chunk.modules.iter().for_each(|id| {
    if let Some(module) = link_stage_output.module_table[*id].as_normal() {
      for name in link_stage_output.symbol_ref_db[module.idx]
        .as_ref()
        .unwrap()
        .ast_scopes
        .root_unresolved_references()
        .keys()
      {
        renamer.reserve(name.to_rstr());
      }
    }
  });

  // Imported symbols are declared in this chunk's scope via import statements.
  chunk.imports_from_other_chunks.iter().flat_map(|(_, items)| items.iter()).for_each(|item| {
    renamer.add_symbol_in_root_scope(item.import_ref);
  });

  chunk.require_binding_names_for_other_chunks = chunk
    .imports_from_other_chunks
    .iter()
    .map(|(id, _)| {
      let name = format!("require_{}", index_chunk_id_to_name[id]);
      let name = legitimize_identifier_name(&name);
      (*id, renamer.create_conflictless_name(&name))
    })
    .collect();

  if let ChunkKind::EntryPoint { module, .. } = chunk.kind {
    link_stage_output.metadata[module].referenced_symbols_by_entry_point_chunk.iter().for_each(
      |symbol_ref| {
        renamer.add_symbol_in_root_scope(*symbol_ref);
      },
    );
  }

  if matches!(format, OutputFormat::Esm) {
    chunk.imports_from_external_modules.iter().for_each(|(module, _)| {
      link_stage_output
        .symbol_ref_db
        .local_db(*module)
        .classic_data
        .iter_enumerated()
        .skip(1)
        .for_each(|(symbol, _)| {
          renamer.add_symbol_in_root_scope((*module, symbol).into());
        });
    });
  }

  chunk
    .modules
    .iter()
    .rev()
    .filter_map(|&id| link_stage_output.module_table[id].as_normal())
    .for_each(|module| {
      module.stmt_infos.iter().for_each(|stmt_info| {
        if stmt_info.is_included {
          for symbol_ref in &stmt_info.declared_symbols {
            renamer.add_symbol_in_root_scope(*symbol_ref);
          }
        }
      });
    });

  chunk.canonical_names = renamer.canonical_names;
}
