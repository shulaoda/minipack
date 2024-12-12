mod module_task;
mod runtime_module_task;
pub mod task_context;

use std::sync::Arc;

use arcstr::ArcStr;
use minipack_common::{ImporterRecord, Module, ModuleIdx, ModuleLoaderMsg, SymbolRefDb};
use oxc_index::IndexVec;
use rustc_hash::FxHashMap;
use task_context::TaskContext;

use crate::types::{IndexEcmaAst, SharedOptions};

pub struct IntermediateNormalModules {
  pub modules: IndexVec<ModuleIdx, Option<Module>>,
  pub importers: IndexVec<ModuleIdx, Vec<ImporterRecord>>,
  pub index_ecma_ast: IndexEcmaAst,
}

impl IntermediateNormalModules {
  pub fn new() -> Self {
    Self {
      modules: IndexVec::new(),
      importers: IndexVec::new(),
      index_ecma_ast: IndexVec::default(),
    }
  }

  pub fn alloc_ecma_module_idx(&mut self) -> ModuleIdx {
    let id = self.modules.push(None);
    self.importers.push(Vec::new());
    id
  }
}

pub struct ModuleLoader {
  options: SharedOptions,
  shared_context: Arc<TaskContext>,
  tx: tokio::sync::mpsc::Sender<ModuleLoaderMsg>,
  rx: tokio::sync::mpsc::Receiver<ModuleLoaderMsg>,
  visited: FxHashMap<ArcStr, ModuleIdx>,
  runtime_id: ModuleIdx,
  remaining: u32,
  intermediate_normal_modules: IntermediateNormalModules,
  symbol_ref_db: SymbolRefDb,
}
