mod bundler_options;
mod chunk;
mod ecmascript;
mod module;
mod module_loader;
mod types;

pub use bundler_options::{
  BundlerOptions, filename_template::FilenameTemplate, input_item::InputItem,
  module_type::ModuleType, normalized_bundler_options::NormalizedBundlerOptions,
  output_format::OutputFormat, platform::Platform,
};

pub use crate::{
  chunk::{Chunk, CrossChunkImportItem, PreliminaryFilename},
  ecmascript::{
    dynamic_import_usage,
    ecma_view::{EcmaModuleAstUsage, EcmaView, EcmaViewMeta},
  },
  module::{Module, external_module::ExternalModule, normal_module::NormalModule},
  module_loader::{
    ModuleLoaderMsg,
    runtime_module_brief::{RUNTIME_MODULE_ID, RuntimeModuleBrief},
    runtime_task_result::RuntimeModuleTaskResult,
    task_result::{EcmaRelated, NormalModuleTaskResult},
  },
  types::{
    asset::Asset,
    ast_scopes::AstScopes,
    chunk_kind::ChunkKind,
    entry_point::{EntryPoint, EntryPointKind},
    import_kind::ImportKind,
    import_record::{ImportRecordMeta, RawImportRecord, ResolvedImportRecord},
    importer_record::ImporterRecord,
    instantiated_chunk::InstantiatedChunk,
    member_expr_ref::MemberExprRef,
    module_id::ModuleId,
    module_info::ModuleInfo,
    named_export::LocalExport,
    named_import::{NamedImport, Specifier},
    namespace_alias::NamespaceAlias,
    output_chunk::{Modules, OutputChunk},
    package_json::PackageJson,
    raw_idx::{
      AssetIdx, ChunkIdx, DUMMY_MODULE_IDX, EcmaAstIdx, ImportRecordIdx, ModuleIdx, StmtInfoIdx,
    },
    rendered_module::RenderedModule,
    resolved_export::ResolvedExport,
    resolved_request_info::ResolvedId,
    side_effects,
    source::Source,
    source_joiner::SourceJoiner,
    stmt_info::{StmtInfo, StmtInfos},
    symbol_name_ref_token::SymbolNameRefToken,
    symbol_or_member_expr_ref::SymbolOrMemberExprRef,
    symbol_ref::SymbolRef,
    symbol_ref_db::{GetLocalDb, SymbolRefDb, SymbolRefDbForModule, SymbolRefFlags},
  },
};
