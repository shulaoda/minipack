mod asset;
mod bundler_options;
mod css;
mod ecmascript;
mod module;
mod module_loader;
mod types;

pub use bundler_options::{
  input_item::InputItem, module_type::ModuleType,
  normalized_bundler_options::NormalizedBundlerOptions, output_format::OutputFormat,
  platform::Platform, resolve_options::ResolveOptions, BundlerOptions,
};

// We don't want internal position adjustment of files affect users, so all items are exported in the root.
pub use crate::{
  asset::asset_view::AssetView,
  css::{
    css_module::CssModule,
    css_module_idx::CssModuleIdx,
    css_view::{CssAssetNameReplacer, CssRenderer, CssView},
  },
  ecmascript::{dynamic_import_usage, ecma_ast_idx::EcmaAstIdx, module_idx::ModuleIdx},
  module::{
    external_module::ExternalModule,
    normal_module::{ModuleRenderArgs, NormalModule},
    Module,
  },
  module_loader::runtime_module_brief::RuntimeModuleBrief,
  types::{
    ast_scopes::AstScopes,
    chunk_idx::ChunkIdx,
    entry_point::EntryPoint,
    exports_kind::ExportsKind,
    external_module_idx::ExternalModuleIdx,
    import_kind::ImportKind,
    import_record::{ImportRecordIdx, ImportRecordMeta, RawImportRecord, ResolvedImportRecord},
    module_def_format::ModuleDefFormat,
    module_id::ModuleId,
    module_info::ModuleInfo,
    module_render_output::ModuleRenderOutput,
    module_table::{IndexExternalModules, IndexModules, ModuleTable},
    named_export::LocalExport,
    named_import::{NamedImport, Specifier},
    namespace_alias::NamespaceAlias,
    output::Output,
    output_chunk::OutputChunk,
    package_json::PackageJson,
    rendered_module::RenderedModule,
    side_effects,
    source_mutation::SourceMutation,
    stmt_info::{DebugStmtInfoForTreeShaking, StmtInfo, StmtInfoIdx, StmtInfoMeta, StmtInfos},
    str_or_bytes::StrOrBytes,
    symbol_or_member_expr_ref::SymbolOrMemberExprRef,
    symbol_ref::SymbolRef,
    symbol_ref_db::{GetLocalDb, SymbolRefDb, SymbolRefDbForModule, SymbolRefFlags},
  },
};
