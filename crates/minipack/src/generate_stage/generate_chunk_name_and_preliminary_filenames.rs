use std::collections::hash_map::Entry;

use arcstr::ArcStr;
use minipack_common::{ChunkIdx, ChunkKind};
use minipack_error::BuildResult;
use minipack_utils::{
  concat_string,
  hash_placeholder::HashPlaceholderGenerator,
  path_ext::PathExt,
  rayon::{IntoParallelRefIterator, ParallelIterator},
  sanitize_file_name::sanitize_file_name,
};
use oxc_index::IndexVec;
use rustc_hash::FxHashMap;
use sugar_path::SugarPath;

use crate::graph::ChunkGraph;

use super::GenerateStage;

impl GenerateStage<'_> {
  /// Notices:
  /// - Should generate filenames that are stable cross builds and os.
  pub async fn generate_chunk_name_and_preliminary_filenames(
    &self,
    chunk_graph: &mut ChunkGraph,
  ) -> BuildResult<FxHashMap<ChunkIdx, ArcStr>> {
    let module_table = &self.link_output.module_table;

    let mut index_chunk_id_to_name = FxHashMap::default();
    let mut index_pre_generated_names: IndexVec<ChunkIdx, ArcStr> = chunk_graph
      .chunk_table
      .par_iter()
      .map(|chunk| {
        if let Some(name) = &chunk.name {
          return name.clone();
        }
        match chunk.kind {
          ChunkKind::EntryPoint { module: entry_module_id, is_user_defined, .. } => {
            let module = &module_table[entry_module_id];
            let generated = if is_user_defined {
              let id = module.id();
              let path = id.as_path();
              path
                .file_stem()
                .and_then(|f| f.to_str())
                .map(ToString::to_string)
                .map_or(arcstr::literal!("input"), |file_name| {
                  sanitize_filename::sanitize(file_name).into()
                })
            } else {
              ArcStr::from(sanitize_file_name(&module.id().as_path().representative_file_name()))
            };
            generated
          }
          ChunkKind::Common => {
            // - rollup use the first entered/last executed module as the `[name]` of common chunks.
            // - esbuild always use 'chunk' as the `[name]`. However we try to make the name more meaningful here.
            let first_executed_non_runtime_module = chunk
              .modules
              .iter()
              .rev()
              .find(|each| **each != self.link_output.runtime_module.id());
            first_executed_non_runtime_module.map_or_else(
              || arcstr::literal!("chunk"),
              |module_id| {
                let module = &module_table[*module_id];
                ArcStr::from(sanitize_file_name(&module.id().as_path().representative_file_name()))
              },
            )
          }
        }
      })
      .collect::<Vec<_>>()
      .into();

    let mut hash_placeholder_generator = HashPlaceholderGenerator::default();

    let create_make_unique_name = |mut used_name_counts: FxHashMap<ArcStr, u32>| {
      move |name: &ArcStr| {
        let mut candidate = name.clone();
        loop {
          match used_name_counts.entry(candidate.clone()) {
            Entry::Occupied(mut occ) => {
              // This name is already used
              let next_count = *occ.get();
              occ.insert(next_count + 1);
              candidate =
                ArcStr::from(concat_string!(name, itoa::Buffer::new().format(next_count)).as_str());
            }
            Entry::Vacant(vac) => {
              // This is the first time we see this name
              let name = vac.key().clone();
              vac.insert(2);
              break name;
            }
          };
        }
      }
    };
    let mut make_unique_name_for_ecma_chunk = create_make_unique_name(FxHashMap::default());

    for chunk_id in &chunk_graph.sorted_chunk_idx_vec {
      let chunk = &mut chunk_graph.chunk_table[*chunk_id];
      if chunk.preliminary_filename.is_some() {
        // Already generated
        continue;
      }

      let pre_generated_chunk_name = &mut index_pre_generated_names[*chunk_id];
      // Notice we didn't used deconflict name here, chunk names are allowed to be duplicated.
      chunk.name = Some(pre_generated_chunk_name.clone());
      index_chunk_id_to_name.insert(*chunk_id, pre_generated_chunk_name.clone());

      let preliminary_filename = chunk.generate_preliminary_filename(
        self.options,
        pre_generated_chunk_name,
        &mut hash_placeholder_generator,
        &mut make_unique_name_for_ecma_chunk,
      )?;

      chunk.absolute_preliminary_filename = Some(
        preliminary_filename
          .absolutize_with(self.options.cwd.join(&self.options.dir))
          .into_os_string()
          .into_string()
          .unwrap_or_else(|input| {
            panic!(
              "Failed to convert {:?} to valid utf8 string",
              std::path::PathBuf::from(input).display()
            );
          }),
      );

      chunk.preliminary_filename = Some(preliminary_filename);
    }
    Ok(index_chunk_id_to_name)
  }
}
