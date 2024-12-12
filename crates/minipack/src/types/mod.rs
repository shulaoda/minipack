pub mod bundle_output;

use std::sync::Arc;

use minipack_common::{
  dynamic_import_usage::DynamicImportExportsUsage, EcmaAstIdx, ModuleIdx, NormalizedBundlerOptions,
};
use minipack_ecmascript::EcmaAst;
use minipack_fs::OsFileSystem;
use minipack_resolver::Resolver;
use oxc_index::IndexVec;
use rustc_hash::FxHashMap;

pub type IndexEcmaAst = IndexVec<EcmaAstIdx, (EcmaAst, ModuleIdx)>;
pub type DynImportUsageMap = FxHashMap<ModuleIdx, DynamicImportExportsUsage>;

pub type SharedResolver = Arc<Resolver<OsFileSystem>>;
pub type SharedOptions = Arc<NormalizedBundlerOptions>;
