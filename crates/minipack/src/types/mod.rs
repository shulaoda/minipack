pub mod bundle_output;
pub mod generator;
pub mod linking_metadata;
pub mod module_factory;

use std::sync::Arc;

use linking_metadata::LinkingMetadata;
use minipack_common::{
  Asset, AssetIdx, EcmaAstIdx, InstantiatedChunk, Module, ModuleIdx, NormalizedBundlerOptions,
  dynamic_import_usage::DynamicImportExportsUsage,
};
use minipack_ecmascript::EcmaAst;
use minipack_fs::OsFileSystem;
use minipack_resolver::Resolver;
use oxc_index::IndexVec;
use rustc_hash::FxHashMap;

pub type IndexAssets = IndexVec<AssetIdx, Asset>;
pub type IndexModules = IndexVec<ModuleIdx, Module>;
pub type IndexEcmaAst = IndexVec<EcmaAstIdx, (EcmaAst, ModuleIdx)>;
pub type LinkingMetadataVec = IndexVec<ModuleIdx, LinkingMetadata>;
pub type IndexInstantiatedChunks = IndexVec<AssetIdx, InstantiatedChunk>;
pub type DynImportUsageMap = FxHashMap<ModuleIdx, DynamicImportExportsUsage>;

pub type SharedResolver = Arc<Resolver<OsFileSystem>>;
pub type SharedOptions = Arc<NormalizedBundlerOptions>;
