use std::hash::Hash;

use itertools::Itertools;
use minipack_common::{AssetIdx, OutputAsset};
use minipack_utils::{
  hash_placeholder::{extract_hash_placeholders, replace_placeholder_with_hash},
  indexmap::FxIndexSet,
  rayon::{IntoParallelIterator, IntoParallelRefIterator, ParallelIterator},
  xxhash::xxhash_base64_url,
};
use minipack_utils::{rayon::IndexedParallelIterator, xxhash::xxhash_with_base};
use oxc_index::{IndexVec, index_vec};
use rustc_hash::FxHashMap;
use xxhash_rust::xxh3::Xxh3;

use crate::types::IndexInstantiatedChunks;

pub fn finalize_assets(instantiated_chunks: IndexInstantiatedChunks) -> Vec<OutputAsset> {
  let asset_idx_by_placeholder = instantiated_chunks
    .iter_enumerated()
    .filter_map(|(asset_idx, asset)| {
      asset.preliminary_filename.hash_placeholder().map(move |placeholders| {
        placeholders.iter().map(move |hash_placeholder| (hash_placeholder.as_str(), asset_idx))
      })
    })
    .flatten()
    .collect::<FxHashMap<_, _>>();

  let index_direct_dependencies = instantiated_chunks
    .par_iter()
    .map(|asset| {
      extract_hash_placeholders(&asset.content)
        .iter()
        .filter_map(|placeholder| asset_idx_by_placeholder.get(placeholder).copied())
        .collect_vec()
    })
    .collect::<Vec<_>>()
    .into();

  // Instead of using `index_direct_dependencies`, we are gonna use `index_transitive_dependencies` to calculate the hash.
  // The reason is that we want to make sure, in `a -> b -> c`, if `c` is changed, not only the direct dependency `b` is changed, but also the indirect dependency `a` is changed.
  let index_transitive_dependencies = collect_transitive_dependencies(&index_direct_dependencies);

  let index_standalone_content_hashes: IndexVec<AssetIdx, String> = instantiated_chunks
    .par_iter()
    .map(|chunk| xxhash_base64_url(chunk.content.as_bytes()))
    .collect::<Vec<_>>()
    .into();

  let index_asset_hashers: IndexVec<AssetIdx, Xxh3> =
    index_vec![Xxh3::default(); instantiated_chunks.len()];

  let index_final_hashes: IndexVec<AssetIdx, String> = index_asset_hashers
    .into_par_iter()
    .enumerate()
    .map(|(asset_idx, mut hasher)| {
      let asset_idx = AssetIdx::from(asset_idx);
      // Start to calculate hash, first we hash itself
      index_standalone_content_hashes[asset_idx].hash(&mut hasher);

      // hash itself's preliminary filename to prevent different chunks that have the same content from having the same hash
      instantiated_chunks[asset_idx].preliminary_filename.hash(&mut hasher);

      let dependencies = &index_transitive_dependencies[asset_idx];
      dependencies.iter().copied().for_each(|dep_id| {
        index_standalone_content_hashes[dep_id].hash(&mut hasher);
      });

      let digested = hasher.digest128();
      xxhash_with_base(&digested.to_le_bytes(), 64)
    })
    .collect::<Vec<_>>()
    .into();

  let final_hashes_by_placeholder = index_final_hashes
    .iter_enumerated()
    .filter_map(|(idx, hash)| {
      let asset = &instantiated_chunks[idx];
      asset.preliminary_filename.hash_placeholder().map(|placeholders| {
        placeholders.iter().map(|placeholder| (placeholder.clone(), &hash[..placeholder.len()]))
      })
    })
    .flatten()
    .collect::<FxHashMap<_, _>>();

  instantiated_chunks
    .into_par_iter()
    .map(|mut asset| {
      let filename = replace_placeholder_with_hash(
        asset.preliminary_filename.as_str(),
        &final_hashes_by_placeholder,
      )
      .into_owned();
      asset.content =
        replace_placeholder_with_hash(&asset.content, &final_hashes_by_placeholder).into_owned();
      asset.finalize(filename)
    })
    .collect::<Vec<_>>()
}

fn collect_transitive_dependencies(
  index_direct_dependencies: &IndexVec<AssetIdx, Vec<AssetIdx>>,
) -> IndexVec<AssetIdx, FxIndexSet<AssetIdx>> {
  fn traverse(
    index: AssetIdx,
    dep_map: &IndexVec<AssetIdx, Vec<AssetIdx>>,
    visited: &mut FxIndexSet<AssetIdx>,
  ) {
    for dep_index in &dep_map[index] {
      if !visited.contains(dep_index) {
        visited.insert(*dep_index);
        traverse(*dep_index, dep_map, visited);
      }
    }
  }

  index_direct_dependencies
    .par_iter()
    .enumerate()
    .map(|(idx, _deps)| {
      let idx = AssetIdx::from(idx);
      let mut visited_deps = FxIndexSet::default();
      traverse(idx, index_direct_dependencies, &mut visited_deps);
      visited_deps
    })
    .collect::<Vec<_>>()
    .into()
}
