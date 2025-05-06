pub mod bundle_output;
pub mod generator;
pub mod linking_metadata;

use std::sync::Arc;

use linking_metadata::LinkingMetadata;
use minipack_common::{
  Asset, AssetIdx, EcmaAstIdx, InstantiatedChunk, Module, ModuleIdx, NormalizedBundlerOptions,
};
use minipack_ecmascript::EcmaAst;
use minipack_fs::OsFileSystem;
use minipack_resolver::Resolver;
use oxc_index::IndexVec;

pub type IndexAssets = IndexVec<AssetIdx, Asset>;
pub type IndexModules = IndexVec<ModuleIdx, Module>;
pub type IndexEcmaAst = IndexVec<EcmaAstIdx, (EcmaAst, ModuleIdx)>;
pub type LinkingMetadataVec = IndexVec<ModuleIdx, LinkingMetadata>;
pub type IndexInstantiatedChunks = IndexVec<AssetIdx, InstantiatedChunk>;

pub type SharedResolver = Arc<Resolver<OsFileSystem>>;
pub type SharedOptions = Arc<NormalizedBundlerOptions>;
