pub mod bundle_output;
pub mod generator;
pub mod linking_metadata;
pub mod module_factory;
pub mod oxc_parse_type;

use std::sync::Arc;

use minipack_common::{
  Asset, AssetIdx, AstScopeIdx, AstScopes, ChunkIdx, EcmaAstIdx, InstantiatedChunk, Module,
  ModuleIdx, NormalizedBundlerOptions, dynamic_import_usage::DynamicImportExportsUsage,
};
use minipack_ecmascript::EcmaAst;
use minipack_fs::OsFileSystem;
use minipack_resolver::Resolver;
use minipack_utils::indexmap::FxIndexSet;
use oxc_index::IndexVec;
use rustc_hash::FxHashMap;

pub type IndexAssets = IndexVec<AssetIdx, Asset>;
pub type IndexModules = IndexVec<ModuleIdx, Module>;
pub type IndexEcmaAst = IndexVec<EcmaAstIdx, (EcmaAst, ModuleIdx)>;
pub type IndexAstScope = IndexVec<AstScopeIdx, AstScopes>;
pub type IndexChunkToAssets = IndexVec<ChunkIdx, FxIndexSet<AssetIdx>>;
pub type IndexInstantiatedChunks = IndexVec<AssetIdx, InstantiatedChunk>;
pub type DynImportUsageMap = FxHashMap<ModuleIdx, DynamicImportExportsUsage>;

pub type SharedResolver = Arc<Resolver<OsFileSystem>>;
pub type SharedOptions = Arc<NormalizedBundlerOptions>;
pub type SharedNormalizedBundlerOptions = Arc<NormalizedBundlerOptions>;
