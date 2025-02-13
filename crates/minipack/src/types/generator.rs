use minipack_common::{
  Chunk, ChunkIdx, InstantiatedChunk, Module, NormalModule, NormalizedBundlerOptions, OutputFormat,
  SymbolRef,
};
use minipack_error::BuildResult;
use minipack_utils::{ecmascript::property_access_str, option_ext::OptionExt, rstr::Rstr};
use rustc_hash::FxHashMap;

use crate::{graph::ChunkGraph, link_stage::LinkStageOutput};

pub struct GenerateContext<'a> {
  pub chunk_idx: ChunkIdx,
  pub chunk: &'a Chunk,
  pub options: &'a NormalizedBundlerOptions,
  pub link_output: &'a LinkStageOutput,
  pub chunk_graph: &'a ChunkGraph,
  pub warnings: Vec<anyhow::Error>,
  pub module_id_to_codegen_ret: Vec<Option<String>>,
}

impl GenerateContext<'_> {
  /// A `SymbolRef` might be identifier or a property access. This function will return correct string pattern for the symbol.
  pub fn finalized_string_pattern_for_symbol_ref(
    &self,
    symbol_ref: SymbolRef,
    cur_chunk_idx: ChunkIdx,
    canonical_names: &FxHashMap<SymbolRef, Rstr>,
  ) -> String {
    let symbol_db = &self.link_output.symbols;
    let canonical_ref = symbol_db.canonical_ref_for(symbol_ref);
    let canonical_symbol = symbol_db.get(canonical_ref);
    let namespace_alias = &canonical_symbol.namespace_alias;
    if let Some(_ns_alias) = namespace_alias {
      // canonical_ref = ns_alias.namespace_ref;
      // canonical_symbol = symbol_db.get(canonical_ref);
      // Not sure if we need to handle this case
      unreachable!("You run into a bug, please report it");
    }

    match self.options.format {
      OutputFormat::Cjs => {
        let chunk_idx_of_canonical_symbol = canonical_symbol.chunk_id.unwrap_or_else(|| {
          // Scoped symbols don't get assigned a `ChunkId`. There are skipped for performance reason, because they are surely
          // belong to the chunk they are declared in and won't link to other chunks.
          let symbol_name = canonical_ref.name(symbol_db);
          panic!("{canonical_ref:?} {symbol_name:?} is not in any chunk, which isn't unexpected");
        });

        let is_symbol_in_other_chunk = cur_chunk_idx != chunk_idx_of_canonical_symbol;
        if is_symbol_in_other_chunk {
          // In cjs output, we need convert the `import { foo } from 'foo'; console.log(foo);`;
          // If `foo` is split into another chunk, we need to convert the code `console.log(foo);` to `console.log(require_xxxx.foo);`
          // instead of keeping `console.log(foo)` as we did in esm output. The reason here is wee need to keep live binding in cjs output.
          let exported_name = &self.chunk_graph.chunk_table[chunk_idx_of_canonical_symbol]
            .exports_to_other_chunks[&canonical_ref];

          let require_binding = &self.chunk_graph.chunk_table[cur_chunk_idx]
            .require_binding_names_for_other_chunks[&chunk_idx_of_canonical_symbol];
          property_access_str(require_binding, exported_name)
        } else {
          symbol_db.canonical_name_for(canonical_ref, canonical_names).to_string()
        }
      }
      _ => symbol_db.canonical_name_for(canonical_ref, canonical_names).to_string(),
    }
  }

  pub fn renderable_ecma_modules(&self) -> impl Iterator<Item = &NormalModule> {
    self.chunk.modules.iter().copied().filter_map(move |id| {
      let module = &self.link_output.modules[id];
      let Module::Normal(module) = module else { return None };
      if !module.is_included() {
        return None;
      }
      let ast = &self.link_output.index_ecma_ast[module.ecma_ast_idx.unpack()].0;
      if ast.program().is_empty() {
        return None;
      }
      Some(&**module)
    })
  }
}

pub struct GenerateOutput {
  pub chunks: Vec<InstantiatedChunk>,
  pub warnings: Vec<anyhow::Error>,
}

pub trait Generator {
  async fn instantiate_chunk(ctx: &mut GenerateContext) -> BuildResult<GenerateOutput>;
}
