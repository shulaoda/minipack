use std::cmp::Ordering;

use itertools::Itertools;
use minipack_common::{Chunk, ChunkKind, Module, ModuleIdx};
use minipack_utils::{bitset::BitSet, rustc_hash::FxHashMapExt};
use oxc_index::IndexVec;
use rustc_hash::FxHashMap;

use crate::graph::ChunkGraph;

use super::GenerateStage;

#[derive(Clone)]
pub struct SplittingInfo {
  bits: BitSet,
  share_count: u32,
}

pub type IndexSplittingInfo = IndexVec<ModuleIdx, SplittingInfo>;

impl GenerateStage {
  pub async fn generate_chunks(&mut self) -> ChunkGraph {
    let entries_len = self.link_stage_output.entry_points.len();
    let max_bit_count = entries_len.try_into().unwrap();

    let mut bits_to_chunk = FxHashMap::with_capacity(entries_len);
    let mut entry_module_to_chunk = FxHashMap::with_capacity(entries_len);
    let mut index_splitting_info = oxc_index::index_vec![SplittingInfo {
      bits: BitSet::new(max_bit_count),
      share_count: 0
    }; self.link_stage_output.module_table.len()];

    let mut chunk_graph = ChunkGraph::new(&self.link_stage_output.module_table, entries_len);

    for (index, entry_point) in self.link_stage_output.entry_points.iter().enumerate() {
      let bit = index.try_into().unwrap();
      let mut bits = BitSet::new(max_bit_count);

      bits.set_bit(bit);

      if let Module::Normal(module) = &self.link_stage_output.module_table[entry_point.idx] {
        let chunk = chunk_graph.add_chunk(Chunk::new(
          entry_point.name.clone(),
          bits.clone(),
          vec![],
          ChunkKind::EntryPoint {
            bit,
            module: entry_point.idx,
            is_user_defined: module.is_user_defined_entry,
          },
        ));
        bits_to_chunk.insert(bits, chunk);
        entry_module_to_chunk.insert(entry_point.idx, chunk);
      }
    }

    // Determine which modules belong to which chunk. A module could belong to multiple chunks.
    self.link_stage_output.entry_points.iter().enumerate().for_each(|(i, entry_point)| {
      let entry_index = i.try_into().unwrap();
      self.determine_reachable_modules_for_entry(
        entry_point.idx,
        entry_index,
        &mut index_splitting_info,
      );
    });

    let mut module_to_assigned =
      oxc_index::index_vec![false; self.link_stage_output.module_table.len()];

    // 1. Assign modules to corresponding chunks
    // 2. Create shared chunks to store modules that belong to multiple chunks.
    for normal_module in self.link_stage_output.module_table.iter().filter_map(Module::as_normal) {
      if !normal_module.meta.is_included() {
        continue;
      }
      if module_to_assigned[normal_module.idx] {
        continue;
      }

      module_to_assigned[normal_module.idx] = true;

      let bits = &index_splitting_info[normal_module.idx].bits;
      debug_assert!(
        !bits.is_empty(),
        "Empty bits means the module is not reachable, so it should bail out with `is_included: false` {:?}",
        normal_module.stable_id
      );

      if let Some(chunk_id) = bits_to_chunk.get(bits).copied() {
        chunk_graph.add_module_to_chunk(normal_module.idx, chunk_id);
      } else {
        let chunk = Chunk::new(None, bits.clone(), vec![], ChunkKind::Common);
        let chunk_id = chunk_graph.add_chunk(chunk);
        chunk_graph.add_module_to_chunk(normal_module.idx, chunk_id);
        bits_to_chunk.insert(bits.clone(), chunk_id);
      }
    }

    // Sort modules in each chunk by execution order
    chunk_graph.chunk_table.iter_mut().for_each(|chunk| {
      chunk.modules.sort_unstable_by_key(|module_id| {
        self.link_stage_output.module_table[*module_id].exec_order()
      });
    });

    chunk_graph
      .chunk_table
      .iter_mut()
      .sorted_by(|a, b| {
        let a_should_be_first = Ordering::Less;
        let b_should_be_first = Ordering::Greater;

        match (&a.kind, &b.kind) {
          (
            ChunkKind::EntryPoint { module: a_module_id, .. },
            ChunkKind::EntryPoint { module: b_module_id, .. },
          ) => self.link_stage_output.module_table[*a_module_id]
            .exec_order()
            .cmp(&self.link_stage_output.module_table[*b_module_id].exec_order()),
          (ChunkKind::EntryPoint { module: a_module_id, .. }, ChunkKind::Common) => {
            let a_module_exec_order =
              self.link_stage_output.module_table[*a_module_id].exec_order();
            let b_chunk_first_module_exec_order =
              self.link_stage_output.module_table[b.modules[0]].exec_order();
            if a_module_exec_order == b_chunk_first_module_exec_order {
              a_should_be_first
            } else {
              a_module_exec_order.cmp(&b_chunk_first_module_exec_order)
            }
          }
          (ChunkKind::Common, ChunkKind::EntryPoint { module: b_module_id, .. }) => {
            let b_module_exec_order =
              self.link_stage_output.module_table[*b_module_id].exec_order();
            let a_chunk_first_module_exec_order =
              self.link_stage_output.module_table[a.modules[0]].exec_order();
            if a_chunk_first_module_exec_order == b_module_exec_order {
              b_should_be_first
            } else {
              a_chunk_first_module_exec_order.cmp(&b_module_exec_order)
            }
          }
          (ChunkKind::Common, ChunkKind::Common) => {
            let a_chunk_first_module_exec_order =
              self.link_stage_output.module_table[a.modules[0]].exec_order();
            let b_chunk_first_module_exec_order =
              self.link_stage_output.module_table[b.modules[0]].exec_order();
            a_chunk_first_module_exec_order.cmp(&b_chunk_first_module_exec_order)
          }
        }
      })
      .enumerate()
      .for_each(|(i, chunk)| {
        chunk.exec_order = i.try_into().expect("Too many chunks, u32 overflowed.");
      });

    // The esbuild using `Chunk#bits` to sorted chunks, but the order of `Chunk#bits` is not stable, eg `BitSet(0) 00000001_00000000` > `BitSet(8) 00000000_00000001`. It couldn't ensure the order of dynamic chunks and common chunks.
    // Consider the compare `Chunk#exec_order` should be faster than `Chunk#bits`, we use `Chunk#exec_order` to sort chunks.
    // Note Here could be make sure the order of chunks.
    // - entry chunks are always before other chunks
    // - static chunks are always before dynamic chunks
    // - other chunks has stable order at per entry chunk level
    // i.e.
    // EntryPoint (is_user_defined: true) < EntryPoint (is_user_defined: false) or Common
    // [order by chunk index]               [order by exec order]
    let sorted_chunk_idx_vec = chunk_graph
      .chunk_table
      .iter_enumerated()
      .sorted_by_key(|(index, chunk)| match &chunk.kind {
        ChunkKind::EntryPoint { is_user_defined, .. } if *is_user_defined => (0, index.raw()),
        _ => (1, chunk.exec_order),
      })
      .map(|(idx, _)| idx)
      .collect::<Vec<_>>();

    chunk_graph.sorted_chunk_idx_vec = sorted_chunk_idx_vec;
    chunk_graph.entry_module_to_chunk = entry_module_to_chunk;

    chunk_graph
  }

  fn determine_reachable_modules_for_entry(
    &self,
    module_idx: ModuleIdx,
    entry_index: u32,
    index_splitting_info: &mut IndexSplittingInfo,
  ) {
    if let Module::Normal(module) = &self.link_stage_output.module_table[module_idx] {
      if !module.meta.is_included() {
        return;
      }

      if index_splitting_info[module_idx].bits.has_bit(entry_index) {
        return;
      }

      index_splitting_info[module_idx].bits.set_bit(entry_index);
      index_splitting_info[module_idx].share_count += 1;

      self.link_stage_output.metadata[module_idx].dependencies.iter().copied().for_each(|idx| {
        self.determine_reachable_modules_for_entry(idx, entry_index, index_splitting_info);
      });
    }
  }
}
