mod module_task;
mod runtime_module_task;
pub mod task_context;

use std::sync::Arc;

use arcstr::ArcStr;
use minipack_common::{
  EntryPoint, ImporterRecord, Module, ModuleIdx, ModuleLoaderMsg, ModuleTable, ModuleType,
  ResolvedId, RuntimeModuleBrief, SymbolRefDb,
};
use minipack_fs::OsFileSystem;
use module_task::ModuleTaskOwner;
use oxc_index::IndexVec;
use rustc_hash::FxHashMap;
use task_context::TaskContext;

use crate::types::{DynImportUsageMap, IndexEcmaAst, SharedOptions, SharedResolver};

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

pub struct ModuleLoaderOutput {
  // Stored all modules
  pub module_table: ModuleTable,
  pub index_ecma_ast: IndexEcmaAst,
  pub symbol_ref_db: SymbolRefDb,
  // Entries that user defined + dynamic import entries
  pub entry_points: Vec<EntryPoint>,
  pub runtime: RuntimeModuleBrief,
  pub warnings: Vec<anyhow::Error>,
  pub dyn_import_usage_map: DynImportUsageMap,
}

impl ModuleLoader {
  pub fn new(
    fs: OsFileSystem,
    options: SharedOptions,
    resolver: SharedResolver,
  ) -> anyhow::Result<Self, Vec<anyhow::Error>> {
    todo!()
  }

  fn try_spawn_new_task(
    &mut self,
    resolved_id: ResolvedId,
    owner: Option<ModuleTaskOwner>,
    is_user_defined_entry: bool,
    assert_module_type: Option<ModuleType>,
  ) -> ModuleIdx {
    todo!()
  }

  pub async fn fetch_all_modules(
    mut self,
    user_defined_entries: Vec<(Option<ArcStr>, ResolvedId)>,
  ) -> anyhow::Result<ModuleLoaderOutput, Vec<anyhow::Error>> {
    if self.options.input.is_empty() {
      Err(vec![anyhow::anyhow!("You must supply options.input to rolldown")])?;
    }

    todo!()
  }
}
