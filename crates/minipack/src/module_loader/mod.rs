mod module_task;
mod runtime_module_task;
pub mod task_context;

use std::sync::Arc;

use arcstr::ArcStr;
use minipack_common::{
  EntryPoint, ImporterRecord, Module, ModuleIdx, ModuleLoaderMsg, ModuleTable, ModuleType,
  ResolvedId, RuntimeModuleBrief, SymbolRefDb, RUNTIME_MODULE_ID,
};
use minipack_error::BuildResult;
use minipack_fs::OsFileSystem;
use module_task::ModuleTaskOwner;
use oxc_index::IndexVec;
use runtime_module_task::RuntimeModuleTask;
use rustc_hash::FxHashMap;
use task_context::TaskContext;
use tokio::sync::mpsc::{Receiver, Sender};

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
    self.importers.push(Vec::new());
    self.modules.push(None)
  }
}

pub struct ModuleLoader {
  tx: Sender<ModuleLoaderMsg>,
  rx: Receiver<ModuleLoaderMsg>,
  remaining: u32,
  options: SharedOptions,
  shared_context: Arc<TaskContext>,
  runtime_id: ModuleIdx,
  symbol_ref_db: SymbolRefDb,
  inm: IntermediateNormalModules,
  visited: FxHashMap<ArcStr, ModuleIdx>,
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
  ) -> BuildResult<Self> {
    // 1024 should be enough for most cases
    // over 1024 pending tasks are insane
    let (tx, rx) = tokio::sync::mpsc::channel(1024);

    let shared_context =
      Arc::new(TaskContext { fs, resolver, options: options.clone(), tx: tx.clone() });

    let mut inm = IntermediateNormalModules::new();

    let runtime_id = inm.alloc_ecma_module_idx();
    let symbol_ref_db = SymbolRefDb::default();

    let task = RuntimeModuleTask::new(runtime_id, tx.clone(), options.clone());
    let visited = FxHashMap::from_iter([(RUNTIME_MODULE_ID.into(), runtime_id)]);

    // task is sync, but execution time is too short at the moment
    // so we are using spawn instead of spawn_blocking here to avoid an additional blocking thread creation within tokio
    let handle = tokio::runtime::Handle::current();
    handle.spawn(async { task.run() });

    Ok(Self {
      tx,
      rx,
      remaining: 1,
      options,
      shared_context,
      runtime_id,
      symbol_ref_db,
      inm,
      visited,
    })
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
  ) -> BuildResult<ModuleLoaderOutput> {
    if self.options.input.is_empty() {
      Err(vec![anyhow::anyhow!("You must supply options.input to rolldown")])?;
    }

    todo!()
  }
}
