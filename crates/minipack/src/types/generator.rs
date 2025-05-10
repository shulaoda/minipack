use minipack_common::{
  Chunk, ChunkIdx, InstantiatedChunk, Module, NormalModule, NormalizedBundlerOptions, OutputFormat,
  SymbolRef,
};
use minipack_utils::{ecmascript::property_access_str, option_ext::OptionExt, rstr::Rstr};
use rustc_hash::FxHashMap;

use crate::{graph::ChunkGraph, link_stage::LinkStageOutput};

pub struct GenerateOutput {
  pub chunks: Vec<InstantiatedChunk>,
  pub warnings: Vec<anyhow::Error>,
}

pub struct GenerateContext<'a> {
  pub chunk: &'a Chunk,
  pub chunk_idx: ChunkIdx,
  pub chunk_graph: &'a ChunkGraph,
  pub link_stage_output: &'a LinkStageOutput,
  pub options: &'a NormalizedBundlerOptions,
  pub module_id_to_codegen_ret: Vec<Option<String>>,
  pub warnings: Vec<anyhow::Error>,
}

impl GenerateContext<'_> {
  /// A `SymbolRef` might be identifier or a property access. This function will return correct string pattern for the symbol.
  pub fn finalized_string_pattern_for_symbol_ref(
    &self,
    symbol_ref: SymbolRef,
    cur_chunk_idx: ChunkIdx,
    canonical_names: &FxHashMap<SymbolRef, Rstr>,
  ) -> String {
    let symbol_db = &self.link_stage_output.symbol_ref_db;
    let canonical_ref = symbol_db.canonical_ref_for(symbol_ref);
    if let OutputFormat::Cjs = self.options.format {
      let canonical_symbol = symbol_db.get(canonical_ref);
      let chunk_idx_of_canonical_symbol = canonical_symbol.chunk_id.unwrap_or_else(|| {
        // Scoped symbols don't get assigned a `ChunkId`. There are skipped for performance reason, because they are surely
        // belong to the chunk they are declared in and won't link to other chunks.
        let symbol_name = canonical_ref.name(symbol_db);
        panic!("{canonical_ref:?} {symbol_name:?} is not in any chunk, which isn't unexpected");
      });

      if cur_chunk_idx != chunk_idx_of_canonical_symbol {
        // In cjs output, we need convert the `import { foo } from 'foo'; console.log(foo);`;
        // If `foo` is split into another chunk, we need to convert the code `console.log(foo);` to `console.log(require_xxxx.foo);`
        // instead of keeping `console.log(foo)` as we did in esm output. The reason here is wee need to keep live binding in cjs output.
        return property_access_str(
          &self.chunk_graph.chunk_table[cur_chunk_idx].require_binding_names_for_other_chunks
            [&chunk_idx_of_canonical_symbol],
          &self.chunk_graph.chunk_table[chunk_idx_of_canonical_symbol].exports_to_other_chunks
            [&canonical_ref],
        );
      }
    }
    symbol_db.canonical_name_for(canonical_ref, canonical_names).to_string()
  }

  pub fn renderable_ecma_modules(&self) -> impl Iterator<Item = &NormalModule> {
    self.chunk.modules.iter().copied().filter_map(move |id| {
      let Module::Normal(module) = &self.link_stage_output.module_table[id] else { return None };
      let ast = &self.link_stage_output.ecma_ast[module.ecma_ast_idx.unpack()].0;
      if !module.is_included() || ast.program().is_empty() { None } else { Some(&**module) }
    })
  }
}
