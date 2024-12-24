mod module_task;
mod runtime_module_task;
pub mod task_context;

use std::sync::Arc;

use arcstr::ArcStr;
use minipack_common::{
  side_effects::{DeterminedSideEffects, HookSideEffects},
  EcmaRelated, EntryPoint, EntryPointKind, ExternalModule, ImportKind, ImportRecordIdx,
  ImporterRecord, Module, ModuleId, ModuleIdx, ModuleLoaderMsg, ModuleTable, ModuleType,
  NormalModuleTaskResult, ResolvedId, ResolvedImportRecord, RuntimeModuleBrief,
  RuntimeModuleTaskResult, SymbolRefDb, SymbolRefDbForModule, RUNTIME_MODULE_ID,
};
use minipack_error::BuildResult;
use minipack_fs::OsFileSystem;
use minipack_utils::{ecmascript::legitimize_identifier_name, rustc_hash::FxHashSetExt};
use module_task::{ModuleTask, ModuleTaskOwner};
use oxc::semantic::{ScopeId, SymbolTable};
use oxc_index::IndexVec;
use runtime_module_task::RuntimeModuleTask;
use rustc_hash::{FxHashMap, FxHashSet};
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
    self.modules.push(None);
    self.importers.push(Vec::new())
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

#[derive(Debug)]
pub struct ModuleLoaderOutput {
  // Stored all modules
  pub module_table: ModuleTable,
  pub index_ecma_ast: IndexEcmaAst,
  pub symbol_ref_db: SymbolRefDb,
  // Entries that user defined + dynamic import entries
  pub entry_points: Vec<EntryPoint>,
  pub runtime_brief: RuntimeModuleBrief,
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

    let visited = FxHashMap::from_iter([(RUNTIME_MODULE_ID.into(), runtime_id)]);

    let task = RuntimeModuleTask::new(runtime_id, tx.clone(), options.clone());

    tokio::spawn(async { task.run() });

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

  pub async fn fetch_all_modules(
    mut self,
    user_defined_entries: Vec<(Option<ArcStr>, ResolvedId)>,
  ) -> BuildResult<ModuleLoaderOutput> {
    if self.options.input.is_empty() {
      Err(vec![anyhow::anyhow!("You must supply options.input to rolldown")])?;
    }

    let entries_count = user_defined_entries.len() + /* runtime */ 1;
    self.inm.modules.reserve(entries_count);
    self.inm.index_ecma_ast.reserve(entries_count);

    // Store the already consider as entry module
    let mut user_defined_entry_ids = FxHashSet::with_capacity(user_defined_entries.len());

    let mut entry_points = user_defined_entries
      .into_iter()
      .map(|(name, info)| {
        let id = self.try_spawn_new_task(info, None, true, None);
        user_defined_entry_ids.insert(id);
        EntryPoint { id, name, kind: EntryPointKind::UserDefined }
      })
      .collect::<Vec<_>>();

    let mut errors: Vec<anyhow::Error> = vec![];
    let mut warnings: Vec<anyhow::Error> = vec![];

    let mut runtime_brief: Option<RuntimeModuleBrief> = None;

    let mut dynamic_import_entry_ids = FxHashSet::default();
    let mut dynamic_import_exports_usage_pairs = vec![];

    while self.remaining > 0 {
      let Some(msg) = self.rx.recv().await else {
        break;
      };

      match msg {
        ModuleLoaderMsg::NormalModuleDone(task_result) => {
          let NormalModuleTaskResult {
            mut ecma_related,
            mut module,
            module_idx,
            resolved_deps,
            raw_import_records,
            warnings: task_result_warnings,
          } = task_result;

          warnings.extend(task_result_warnings);

          let mut dynamic_import_rec_exports_usage = ecma_related
            .as_mut()
            .map(|item| std::mem::take(&mut item.dynamic_import_rec_exports_usage))
            .unwrap_or_default();

          let import_records = raw_import_records
            .into_iter_enumerated()
            .zip(resolved_deps)
            .map(|((rec_idx, raw_rec), info)| {
              let normal_module = module.as_normal().unwrap();
              let owner = ModuleTaskOwner::new(
                normal_module.source.clone(),
                normal_module.stable_id.as_str().into(),
                raw_rec.span,
              );
              let id = self.try_spawn_new_task(
                info,
                Some(owner),
                false,
                raw_rec.asserted_module_type.clone(),
              );
              // Dynamic imported module will be considered as an entry
              self.inm.importers[id].push(ImporterRecord {
                kind: raw_rec.kind,
                importer_path: ModuleId::new(module.id()),
              });
              // defer usage merging, since we only have one consumer, we should keep action during fetching as simple
              // as possible
              if let Some(usage) = dynamic_import_rec_exports_usage.remove(&rec_idx) {
                dynamic_import_exports_usage_pairs.push((id, usage));
              }
              if matches!(raw_rec.kind, ImportKind::DynamicImport)
                && !user_defined_entry_ids.contains(&id)
              {
                dynamic_import_entry_ids.insert(id);
              }
              raw_rec.into_resolved(id)
            })
            .collect::<IndexVec<ImportRecordIdx, _>>();

          module.set_import_records(import_records);

          if let Some(EcmaRelated { ast, symbols, .. }) = ecma_related {
            let ast_idx = self.inm.index_ecma_ast.push((ast, module.idx()));
            module.set_ecma_ast_idx(ast_idx);
            self.symbol_ref_db.store_local_db(module_idx, symbols);
          }
          self.inm.modules[module_idx] = Some(module);
          self.remaining -= 1;
        }
        ModuleLoaderMsg::RuntimeModuleDone(task_result) => {
          let RuntimeModuleTaskResult {
            local_symbol_ref_db,
            mut module,
            runtime,
            ast,
            raw_import_records,
            resolved_deps,
          } = task_result;
          let import_records: IndexVec<ImportRecordIdx, ResolvedImportRecord> = raw_import_records
            .into_iter_enumerated()
            .zip(resolved_deps)
            .map(|((_rec_idx, raw_rec), info)| {
              let id =
                self.try_spawn_new_task(info, None, false, raw_rec.asserted_module_type.clone());
              // Dynamic imported module will be considered as an entry
              self.inm.importers[id]
                .push(ImporterRecord { kind: raw_rec.kind, importer_path: module.id.clone() });

              if matches!(raw_rec.kind, ImportKind::DynamicImport)
                && !user_defined_entry_ids.contains(&id)
              {
                dynamic_import_entry_ids.insert(id);
              }
              raw_rec.into_resolved(id)
            })
            .collect::<IndexVec<ImportRecordIdx, _>>();

          let ast_idx = self.inm.index_ecma_ast.push((ast, module.idx));

          runtime_brief = Some(runtime);
          module.ecma_ast_idx = Some(ast_idx);
          module.import_records = import_records;

          self.inm.modules[self.runtime_id] = Some(module.into());
          self.symbol_ref_db.store_local_db(self.runtime_id, local_symbol_ref_db);

          self.remaining -= 1;
        }
        ModuleLoaderMsg::BuildErrors(e) => {
          errors.extend(e);
          self.remaining -= 1;
        }
      }
    }

    if !errors.is_empty() {
      Err(errors)?;
    }

    let dyn_import_usage_map = dynamic_import_exports_usage_pairs.into_iter().fold(
      FxHashMap::default(),
      |mut acc, (idx, usage)| {
        match acc.entry(idx) {
          std::collections::hash_map::Entry::Vacant(vac) => {
            vac.insert(usage);
          }
          std::collections::hash_map::Entry::Occupied(mut occ) => {
            occ.get_mut().merge(usage);
          }
        };
        acc
      },
    );

    let modules: IndexVec<ModuleIdx, Module> = self
      .inm
      .modules
      .into_iter()
      .enumerate()
      .map(|(id, module)| {
        let mut module = module.expect("Module tasks did't complete as expected");

        if let Some(module) = module.as_normal_mut() {
          let id = ModuleIdx::from(id);
          // Note: (Compat to rollup)
          // The `dynamic_importers/importers` should be added after `module_parsed` hook.
          let importers = std::mem::take(&mut self.inm.importers[id]);
          for importer in &importers {
            if importer.kind.is_static() {
              module.importers.insert(importer.importer_path.clone());
            } else {
              module.dynamic_importers.insert(importer.importer_path.clone());
            }
          }
        }
        module
      })
      .collect();

    // if `inline_dynamic_imports` is set to be true, here we should not put dynamic imports to entries
    if !self.options.inline_dynamic_imports {
      let mut dynamic_import_entry_ids = dynamic_import_entry_ids.into_iter().collect::<Vec<_>>();
      dynamic_import_entry_ids.sort_unstable_by_key(|id| modules[*id].stable_id());

      entry_points.extend(dynamic_import_entry_ids.into_iter().map(|id| EntryPoint {
        name: None,
        id,
        kind: EntryPointKind::DynamicImport,
      }));
    }

    let runtime_brief =
      runtime_brief.expect("Failed to find runtime module. This should not happen");

    Ok(ModuleLoaderOutput {
      module_table: ModuleTable { modules },
      symbol_ref_db: self.symbol_ref_db,
      index_ecma_ast: self.inm.index_ecma_ast,
      entry_points,
      runtime_brief,
      warnings,
      dyn_import_usage_map,
    })
  }

  fn try_spawn_new_task(
    &mut self,
    resolved_id: ResolvedId,
    owner: Option<ModuleTaskOwner>,
    is_user_defined_entry: bool,
    assert_module_type: Option<ModuleType>,
  ) -> ModuleIdx {
    match self.visited.entry(resolved_id.id.clone()) {
      std::collections::hash_map::Entry::Occupied(visited) => *visited.get(),
      std::collections::hash_map::Entry::Vacant(not_visited) => {
        if resolved_id.is_external {
          let idx = self.inm.alloc_ecma_module_idx();
          not_visited.insert(idx);
          let external_module_side_effects =
            if let Some(hook_side_effects) = resolved_id.side_effects {
              match hook_side_effects {
                HookSideEffects::True => DeterminedSideEffects::UserDefined(true),
                HookSideEffects::False => DeterminedSideEffects::UserDefined(false),
                HookSideEffects::NoTreeshake => DeterminedSideEffects::NoTreeshake,
              }
            } else {
              DeterminedSideEffects::NoTreeshake
            };

          self.symbol_ref_db.store_local_db(
            idx,
            SymbolRefDbForModule::new(SymbolTable::default(), idx, ScopeId::new(0)),
          );
          let symbol_ref = self.symbol_ref_db.create_facade_root_symbol_ref(
            idx,
            &legitimize_identifier_name(resolved_id.id.as_str()),
          );
          let ext = ExternalModule::new(
            idx,
            ArcStr::clone(&resolved_id.id),
            external_module_side_effects,
            symbol_ref,
          );
          self.inm.modules[idx] = Some(ext.into());
          idx
        } else {
          let idx = self.inm.alloc_ecma_module_idx();
          not_visited.insert(idx);
          self.remaining += 1;

          let task = ModuleTask::new(
            self.shared_context.clone(),
            idx,
            owner,
            resolved_id,
            is_user_defined_entry,
            assert_module_type,
          );
          tokio::spawn(task.run());
          idx
        }
      }
    }
  }
}
