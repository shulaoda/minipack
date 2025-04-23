use std::collections::hash_map::Entry;
use std::sync::Arc;

use arcstr::ArcStr;
use minipack_common::StmtInfoIdx;
use minipack_common::{
  DUMMY_MODULE_IDX, EcmaRelated, EntryPoint, EntryPointKind, ExternalModule, ImportKind,
  ImportRecordIdx, ImportRecordMeta, ImporterRecord, Module, ModuleId, ModuleIdx, ModuleLoaderMsg,
  ModuleType, NormalModuleTaskResult, RUNTIME_MODULE_ID, ResolvedId, ResolvedImportRecord,
  RuntimeModuleBrief, RuntimeModuleTaskResult, SymbolRefDb, SymbolRefDbForModule,
  side_effects::DeterminedSideEffects,
};
use minipack_error::BuildResult;
use minipack_fs::OsFileSystem;
use minipack_utils::rstr::Rstr;
use minipack_utils::rustc_hash::FxHashSetExt;
use oxc::semantic::{ScopeId, Scoping};
use oxc_index::IndexVec;
use rustc_hash::{FxHashMap, FxHashSet};
use tokio::sync::mpsc::Receiver;

use super::module_task::ModuleTask;
use super::runtime_module_task::RuntimeModuleTask;
use super::task_context::TaskContext;

use crate::types::{DynImportUsageMap, IndexEcmaAst, IndexModules, SharedOptions, SharedResolver};
use crate::utils::ecmascript::legitimize_identifier_name;

pub struct IntermediateNormalModules {
  pub modules: IndexVec<ModuleIdx, Option<Module>>,
  pub importers: IndexVec<ModuleIdx, Vec<ImporterRecord>>,
  pub index_ecma_ast: IndexEcmaAst,
}

impl IntermediateNormalModules {
  pub fn new() -> Self {
    Self { modules: IndexVec::new(), importers: IndexVec::new(), index_ecma_ast: IndexVec::new() }
  }

  pub fn alloc_ecma_module_idx(&mut self) -> ModuleIdx {
    self.modules.push(None);
    self.importers.push(Vec::new())
  }
}

pub struct ModuleLoader {
  rx: Receiver<ModuleLoaderMsg>,
  remaining: u32,
  shared_context: Arc<TaskContext>,
  runtime_idx: ModuleIdx,
  symbols: SymbolRefDb,
  inm: IntermediateNormalModules,
  visited: FxHashMap<ArcStr, ModuleIdx>,
}

#[derive(Debug)]
pub struct ModuleLoaderOutput {
  // Stored all modules
  pub symbols: SymbolRefDb,
  pub module_table: IndexModules,
  pub index_ecma_ast: IndexEcmaAst,
  // Entries that user defined + dynamic import entries
  pub entry_points: Vec<EntryPoint>,
  pub runtime_module: RuntimeModuleBrief,
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

    let runtime_idx = inm.alloc_ecma_module_idx();
    let symbols = SymbolRefDb::default();

    let visited = FxHashMap::from_iter([(RUNTIME_MODULE_ID.into(), runtime_idx)]);

    let task = RuntimeModuleTask::new(runtime_idx, tx.clone(), options.clone());

    tokio::spawn(async { task.run() });

    Ok(Self { rx, remaining: 1, shared_context, runtime_idx, symbols, inm, visited })
  }

  pub async fn fetch_all_modules(
    mut self,
    user_defined_entries: Vec<(Option<ArcStr>, ResolvedId)>,
  ) -> BuildResult<ModuleLoaderOutput> {
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
        EntryPoint { id, name, kind: EntryPointKind::UserDefined, related_stmt_infos: vec![] }
      })
      .collect::<Vec<_>>();

    let mut errors: Vec<anyhow::Error> = vec![];
    let mut warnings: Vec<anyhow::Error> = vec![];

    let mut runtime_module: Option<RuntimeModuleBrief> = None;

    let mut dynamic_import_entry_ids: FxHashMap<ModuleIdx, Vec<(ModuleIdx, StmtInfoIdx)>> =
      FxHashMap::default();
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
            resolved_deps,
            raw_import_records,
            warnings: task_result_warnings,
          } = task_result;

          warnings.extend(task_result_warnings);

          let mut dynamic_import_rec_exports_usage = ecma_related
            .as_mut()
            .map(|item| std::mem::take(&mut item.dynamic_import_exports_usage))
            .unwrap_or_default();

          let import_records = raw_import_records
            .into_iter_enumerated()
            .zip(resolved_deps)
            .map(|((rec_idx, raw_rec), info)| {
              if raw_rec.meta.contains(ImportRecordMeta::IS_DUMMY) {
                return raw_rec.into_resolved(DUMMY_MODULE_IDX);
              }
              let normal_module = module.as_normal().unwrap();
              let owner = normal_module.stable_id.as_str().into();
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
                match dynamic_import_entry_ids.entry(id) {
                  Entry::Vacant(vac) => match raw_rec.related_stmt_info_idx {
                    Some(stmt_info_idx) => {
                      vac.insert(vec![(module.idx(), stmt_info_idx)]);
                    }
                    None => {
                      vac.insert(vec![]);
                    }
                  },
                  Entry::Occupied(mut occ) => {
                    if let Some(stmt_info_idx) = raw_rec.related_stmt_info_idx {
                      occ.get_mut().push((module.idx(), stmt_info_idx));
                    }
                  }
                }
              }
              raw_rec.into_resolved(id)
            })
            .collect::<IndexVec<ImportRecordIdx, _>>();

          module.set_import_records(import_records);

          let module_idx = module.idx();
          if let Some(EcmaRelated { ast, symbols, .. }) = ecma_related {
            module.set_ecma_ast_idx(self.inm.index_ecma_ast.push((ast, module_idx)));
            self.symbols.store_local_db(module_idx, symbols);
          }

          self.inm.modules[module_idx] = Some(module);
          self.remaining -= 1;
        }
        ModuleLoaderMsg::RuntimeModuleDone(task_result) => {
          let RuntimeModuleTaskResult {
            mut module,
            runtime,
            ast,
            symbols,
            raw_import_records,
            resolved_deps,
          } = task_result;
          let import_records: IndexVec<ImportRecordIdx, ResolvedImportRecord> = raw_import_records
            .into_iter()
            .zip(resolved_deps)
            .map(|(raw_rec, info)| {
              let id =
                self.try_spawn_new_task(info, None, false, raw_rec.asserted_module_type.clone());
              // Dynamic imported module will be considered as an entry
              self.inm.importers[id]
                .push(ImporterRecord { kind: raw_rec.kind, importer_path: module.id.clone() });

              if matches!(raw_rec.kind, ImportKind::DynamicImport)
                && !user_defined_entry_ids.contains(&id)
              {
                match dynamic_import_entry_ids.entry(id) {
                  Entry::Vacant(vac) => match raw_rec.related_stmt_info_idx {
                    Some(stmt_info_idx) => {
                      vac.insert(vec![(module.idx, stmt_info_idx)]);
                    }
                    None => {
                      vac.insert(vec![]);
                    }
                  },
                  Entry::Occupied(mut occ) => {
                    if let Some(stmt_info_idx) = raw_rec.related_stmt_info_idx {
                      occ.get_mut().push((module.idx, stmt_info_idx));
                    }
                  }
                }
              }
              raw_rec.into_resolved(id)
            })
            .collect::<IndexVec<ImportRecordIdx, _>>();

          let ast_idx = self.inm.index_ecma_ast.push((ast, module.idx));

          module.ecma_ast_idx = Some(ast_idx);
          module.import_records = import_records;

          runtime_module = Some(runtime);

          self.inm.modules[self.runtime_idx] = Some(module.into());
          self.symbols.store_local_db(self.runtime_idx, symbols);

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
          Entry::Vacant(vac) => {
            vac.insert(usage);
          }
          Entry::Occupied(mut occ) => {
            occ.get_mut().merge(usage);
          }
        };
        acc
      },
    );

    let module_table: IndexVec<ModuleIdx, Module> = self
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

    let mut dynamic_import_entry_ids = dynamic_import_entry_ids.into_iter().collect::<Vec<_>>();
    dynamic_import_entry_ids.sort_unstable_by_key(|(id, _)| module_table[*id].stable_id());

    entry_points.extend(dynamic_import_entry_ids.into_iter().map(|(id, related_stmt_infos)| {
      EntryPoint { name: None, id, kind: EntryPointKind::DynamicImport, related_stmt_infos }
    }));

    let runtime_module =
      runtime_module.expect("Failed to find runtime module. This should not happen");

    Ok(ModuleLoaderOutput {
      module_table,
      symbols: self.symbols,
      index_ecma_ast: self.inm.index_ecma_ast,
      entry_points,
      runtime_module,
      warnings,
      dyn_import_usage_map,
    })
  }

  fn try_spawn_new_task(
    &mut self,
    resolved_id: ResolvedId,
    owner: Option<Rstr>,
    is_user_defined_entry: bool,
    assert_module_type: Option<ModuleType>,
  ) -> ModuleIdx {
    match self.visited.entry(resolved_id.id.clone()) {
      Entry::Occupied(visited) => *visited.get(),
      Entry::Vacant(not_visited) => {
        let idx = self.inm.alloc_ecma_module_idx();

        if resolved_id.is_external {
          self.symbols.store_local_db(
            idx,
            SymbolRefDbForModule::new(idx, Scoping::default(), ScopeId::new(0)),
          );

          let symbol_ref = self.symbols.create_facade_root_symbol_ref(
            idx,
            &legitimize_identifier_name(resolved_id.id.as_str()),
          );

          self.inm.modules[idx] = Some(Module::external(ExternalModule::new(
            idx,
            resolved_id.id,
            DeterminedSideEffects::NoTreeshake,
            symbol_ref,
          )));
        } else {
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
        }

        *not_visited.insert(idx)
      }
    }
  }
}
