use minipack_common::{EntryPoint, ModuleTable, RuntimeModuleBrief, SymbolRefDb};
use minipack_fs::OsFileSystem;

use crate::types::{DynImportUsageMap, IndexEcmaAst, SharedOptions, SharedResolver};

pub struct ScanStage {
  fs: OsFileSystem,
  options: SharedOptions,
  resolver: SharedResolver,
  // plugin_driver: SharedPluginDriver,
}

#[derive(Debug)]
pub struct ScanStageOutput {
  pub entry_points: Vec<EntryPoint>,
  pub module_table: ModuleTable,
  pub symbol_ref_db: SymbolRefDb,
  pub runtime: RuntimeModuleBrief,
  pub index_ecma_ast: IndexEcmaAst,
  pub dyn_import_usage_map: DynImportUsageMap,
}

impl ScanStage {
  pub fn new(fs: OsFileSystem, options: SharedOptions, resolver: SharedResolver) -> Self {
    Self { fs, options, resolver }
  }

  pub async fn scan(&mut self) -> anyhow::Result<ScanStageOutput> {
    if self.options.input.is_empty() {
      return Err(anyhow::format_err!("You must supply options.input to rolldown").into());
    }

    todo!()
  }
}
