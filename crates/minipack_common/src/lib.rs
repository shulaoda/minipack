mod asset;
mod bundler_options;
mod chunk;
mod css;
mod ecmascript;
mod module;
mod module_loader;
mod types;

pub use bundler_options::{
  es_target::ESTarget,
  filename_template::{FileNameRenderOptions, FilenameTemplate},
  input_item::InputItem,
  module_type::ModuleType,
  normalized_bundler_options::NormalizedBundlerOptions,
  output_exports::OutputExports,
  output_format::OutputFormat,
  platform::Platform,
  resolve_options::ResolveOptions,
  BundlerOptions,
};

// We don't want internal position adjustment of files affect users, so all items are exported in the root.
pub use crate::{
  asset::AssetView,
  chunk::{
    types::{
      cross_chunk_import_item::CrossChunkImportItem, preliminary_filename::PreliminaryFilename,
    },
    Chunk,
  },
  css::{CssAssetNameReplacer, CssRenderer, CssView},
  ecmascript::{
    dynamic_import_usage,
    ecma_ast_idx::EcmaAstIdx,
    ecma_view::{
      generate_replace_this_expr_map, EcmaModuleAstUsage, EcmaView, EcmaViewMeta,
      ImportMetaRolldownAssetReplacer, ThisExprReplaceKind,
    },
    module_idx::{ModuleIdx, DUMMY_MODULE_IDX},
    node_builtin_modules::is_existing_node_builtin_modules,
  },
  module::{
    external_module::ExternalModule,
    normal_module::{ModuleRenderArgs, NormalModule},
    Module,
  },
  module_loader::{
    runtime_module_brief::{RuntimeModuleBrief, RUNTIME_MODULE_ID},
    runtime_task_result::RuntimeModuleTaskResult,
    task_result::{EcmaRelated, NormalModuleTaskResult},
    ModuleLoaderMsg,
  },
  types::{
    asset::Asset,
    asset_idx::AssetIdx,
    asset_meta::InstantiationKind,
    ast_scopes::AstScopes,
    chunk_idx::ChunkIdx,
    chunk_kind::ChunkKind,
    entry_point::{EntryPoint, EntryPointKind},
    exports_kind::ExportsKind,
    import_kind::ImportKind,
    import_record::{ImportRecordIdx, ImportRecordMeta, RawImportRecord, ResolvedImportRecord},
    importer_record::ImporterRecord,
    instantiated_chunk::InstantiatedChunk,
    interop::Interop,
    member_expr_ref::MemberExprRef,
    module_def_format::ModuleDefFormat,
    module_id::ModuleId,
    module_info::ModuleInfo,
    named_export::LocalExport,
    named_import::{NamedImport, Specifier},
    namespace_alias::NamespaceAlias,
    output::{Output, OutputAsset},
    output_chunk::{Modules, OutputChunk},
    package_json::PackageJson,
    rendered_module::RenderedModule,
    resolved_export::ResolvedExport,
    resolved_request_info::ResolvedId,
    rollup_pre_rendered_chunk::RollupPreRenderedChunk,
    rollup_rendered_chunk::RollupRenderedChunk,
    side_effects,
    source::Source,
    source_joiner::SourceJoiner,
    source_mutation::SourceMutation,
    stmt_info::{StmtInfo, StmtInfoIdx, StmtInfoMeta, StmtInfos},
    str_or_bytes::StrOrBytes,
    symbol_name_ref_token::SymbolNameRefToken,
    symbol_or_member_expr_ref::SymbolOrMemberExprRef,
    symbol_ref::SymbolRef,
    symbol_ref_db::{GetLocalDb, SymbolRefDb, SymbolRefDbForModule, SymbolRefFlags},
    wrap_kind::WrapKind,
  },
};
