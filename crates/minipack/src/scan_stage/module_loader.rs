use std::collections::hash_map::Entry;
use std::sync::Arc;

use arcstr::ArcStr;
use minipack_common::{
  EntryPoint, EntryPointKind, ExternalModule, ImportRecordIdx, ImporterRecord, Module, ModuleIdx,
  ModuleLoaderMsg, NormalModuleTaskResult, RUNTIME_MODULE_ID, ResolvedId, RuntimeModuleBrief,
  RuntimeModuleTaskResult, SymbolRefDb, SymbolRefDbForModule,
};
use minipack_error::BuildResult;
use minipack_fs::OsFileSystem;
use minipack_utils::rstr::Rstr;
use minipack_utils::rustc_hash::FxHashSetExt;
use oxc::semantic::{ScopeId, Scoping};
use oxc_index::IndexVec;
use rustc_hash::{FxHashMap, FxHashSet};
use tokio::sync::mpsc::Receiver;

use crate::types::{IndexEcmaAst, IndexModules, SharedOptions, SharedResolver};
use crate::utils::ecmascript::legitimize_identifier_name;

use super::module_task::{ModuleTask, TaskContext};
use super::runtime_module_task::RuntimeModuleTask;

pub struct IntermediateNormalModules {
  pub ecma_ast: IndexEcmaAst,
  pub module_table: IndexVec<ModuleIdx, Option<Module>>,
  pub importer_record: IndexVec<ModuleIdx, Vec<ImporterRecord>>,
}

impl IntermediateNormalModules {
  pub fn new() -> Self {
    Self {
      ecma_ast: IndexVec::new(),
      module_table: IndexVec::new(),
      importer_record: IndexVec::new(),
    }
  }

  pub fn alloc_ecma_module_idx(&mut self) -> ModuleIdx {
    self.module_table.push(None);
    self.importer_record.push(Vec::new())
  }
}

pub struct ModuleLoader {
  rx: Receiver<ModuleLoaderMsg>,
  inm: IntermediateNormalModules,
  remaining: u32,
  runtime_idx: ModuleIdx,
  symbol_ref_db: SymbolRefDb,
  shared_context: Arc<TaskContext>,
  visited: FxHashMap<ArcStr, ModuleIdx>,
}

#[derive(Debug)]
pub struct ModuleLoaderOutput {
  pub ecma_ast: IndexEcmaAst,
  pub module_table: IndexModules,
  pub symbol_ref_db: SymbolRefDb,
  pub entry_points: Vec<EntryPoint>,
  pub runtime_module: RuntimeModuleBrief,
  pub warnings: Vec<anyhow::Error>,
}

impl ModuleLoader {
  pub fn new(
    fs: OsFileSystem,
    options: SharedOptions,
    resolver: SharedResolver,
  ) -> BuildResult<Self> {
    let (tx, rx) = tokio::sync::mpsc::channel(1024);

    let mut inm = IntermediateNormalModules::new();

    let runtime_idx = inm.alloc_ecma_module_idx();
    let symbol_ref_db = SymbolRefDb::default();

    let visited = FxHashMap::from_iter([(RUNTIME_MODULE_ID.into(), runtime_idx)]);
    let shared_context = Arc::new(TaskContext { fs, resolver, options, tx: tx.clone() });

    let task = RuntimeModuleTask::new(runtime_idx, tx.clone());
    tokio::spawn(async { task.run() });

    Ok(Self { rx, remaining: 1, shared_context, runtime_idx, symbol_ref_db, inm, visited })
  }

  pub async fn fetch_all_modules(
    mut self,
    user_defined_entries: Vec<(Option<ArcStr>, ResolvedId)>,
  ) -> BuildResult<ModuleLoaderOutput> {
    let entries_count = user_defined_entries.len();
    let modules_count = entries_count + /* runtime */ 1;

    self.inm.ecma_ast.reserve(modules_count);
    self.inm.module_table.reserve(modules_count);

    let mut entry_points = Vec::with_capacity(entries_count);
    let mut user_defined_entry_ids = FxHashSet::with_capacity(entries_count);

    for (name, resolved_id) in user_defined_entries.into_iter() {
      let idx = self.try_spawn_new_task(None, resolved_id, true);
      user_defined_entry_ids.insert(idx);
      entry_points.push(EntryPoint { idx, name, kind: EntryPointKind::UserDefined });
    }

    let mut warnings = vec![];
    let mut runtime_module = None;

    let mut dynamic_import_entry_ids = user_defined_entry_ids.clone();

    while self.remaining > 0 {
      let Some(msg) = self.rx.recv().await else {
        break;
      };

      match msg {
        ModuleLoaderMsg::NormalModuleDone(NormalModuleTaskResult {
          mut module,
          ecma_related,
          resolved_deps,
          raw_import_records,
          warnings: task_result_warnings,
        }) => {
          let normal_module = module.as_normal_mut().unwrap();
          let mut import_records =
            IndexVec::<ImportRecordIdx, _>::with_capacity(raw_import_records.len());

          for (import_record, resolved_id) in raw_import_records.into_iter().zip(resolved_deps) {
            let owner = normal_module.stable_id.as_str().into();
            let idx = self.try_spawn_new_task(Some(owner), resolved_id, false);
            if import_record.kind.is_dynamic() && !dynamic_import_entry_ids.contains(&idx) {
              dynamic_import_entry_ids.insert(idx);
              entry_points.push(EntryPoint {
                idx,
                name: None,
                kind: EntryPointKind::DynamicImport,
              });
            }

            self.inm.importer_record[idx].push(ImporterRecord {
              kind: import_record.kind,
              importer_path: normal_module.id.clone(),
            });

            import_records.push(import_record.into_resolved(idx));
          }

          warnings.extend(task_result_warnings);
          normal_module.import_records = import_records;

          let module_idx = normal_module.idx;
          if let Some(ecma_related) = ecma_related {
            module.set_ecma_ast_idx(self.inm.ecma_ast.push((ecma_related.ast, module_idx)));
            self.symbol_ref_db.store_local_db(module_idx, ecma_related.symbols);
          }

          self.inm.module_table[module_idx] = Some(module);
          self.remaining -= 1;
        }
        ModuleLoaderMsg::RuntimeModuleDone(RuntimeModuleTaskResult {
          mut module,
          ast,
          runtime,
          symbols,
        }) => {
          let ecma_ast_idx = self.inm.ecma_ast.push((ast, module.idx));

          runtime_module = Some(runtime);
          module.ecma_ast_idx = Some(ecma_ast_idx);

          self.inm.module_table[self.runtime_idx] = Some(module.into());
          self.symbol_ref_db.store_local_db(self.runtime_idx, symbols);

          self.remaining -= 1;
        }
        ModuleLoaderMsg::BuildErrors(errors) => {
          self.rx.close();
          Err(errors)?;
        }
      }
    }

    let module_table = self.inm.module_table.into_iter().flatten().collect();
    let runtime_module = runtime_module.expect("Failed to find runtime module.");

    Ok(ModuleLoaderOutput {
      entry_points,
      module_table,
      runtime_module,
      ecma_ast: self.inm.ecma_ast,
      symbol_ref_db: self.symbol_ref_db,
      warnings,
    })
  }

  fn try_spawn_new_task(
    &mut self,
    owner: Option<Rstr>,
    resolved_id: ResolvedId,
    is_user_defined_entry: bool,
  ) -> ModuleIdx {
    match self.visited.entry(resolved_id.id.clone()) {
      Entry::Occupied(visited) => *visited.get(),
      Entry::Vacant(not_visited) => {
        let idx = self.inm.alloc_ecma_module_idx();
        if resolved_id.is_external {
          self.symbol_ref_db.store_local_db(
            idx,
            SymbolRefDbForModule::new(idx, Scoping::default(), ScopeId::new(0)),
          );

          let name = legitimize_identifier_name(resolved_id.id.as_str());
          let namespace_ref = self.symbol_ref_db.create_facade_root_symbol_ref(idx, &name);
          let module = Box::new(ExternalModule::new(idx, resolved_id.id, namespace_ref));

          self.inm.module_table[idx] = Some(Module::External(module));
        } else {
          let task = ModuleTask::new(
            self.shared_context.clone(),
            idx,
            owner,
            resolved_id,
            is_user_defined_entry,
          );
          tokio::spawn(task.run());
          self.remaining += 1;
        }
        *not_visited.insert(idx)
      }
    }
  }
}
