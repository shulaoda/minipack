use std::{
  borrow::Cow,
  path::{Path, PathBuf},
};

pub mod types;

use arcstr::ArcStr;

use minipack_utils::{
  bitset::BitSet, hash_placeholder::HashPlaceholderGenerator, indexmap::FxIndexMap,
  path_ext::PathExt, rstr::Rstr,
};
use oxc_index::IndexVec;
use rustc_hash::FxHashMap;
use sugar_path::SugarPath;

use crate::{
  ChunkIdx, ChunkKind, FilenameTemplate, Module, ModuleIdx, NamedImport, NormalModule,
  NormalizedBundlerOptions, RollupPreRenderedChunk, SymbolNameRefToken, SymbolRef,
};

use self::types::{
  cross_chunk_import_item::CrossChunkImportItem, preliminary_filename::PreliminaryFilename,
};

#[derive(Debug, Default)]
pub struct Chunk {
  pub exec_order: u32,
  pub kind: ChunkKind,
  pub modules: Vec<ModuleIdx>,
  pub name: Option<ArcStr>,
  pub pre_rendered_chunk: Option<RollupPreRenderedChunk>,
  pub preliminary_filename: Option<PreliminaryFilename>,
  pub absolute_preliminary_filename: Option<String>,
  pub css_preliminary_filename: Option<PreliminaryFilename>,
  pub css_absolute_preliminary_filename: Option<String>,
  pub asset_preliminary_filenames: FxIndexMap<ModuleIdx, PreliminaryFilename>,
  pub asset_absolute_preliminary_filenames: FxIndexMap<ModuleIdx, String>,
  pub canonical_names: FxHashMap<SymbolRef, Rstr>,
  pub canonical_name_by_token: FxHashMap<SymbolNameRefToken, Rstr>,
  // Sorted by Module#stable_id of modules in the chunk
  pub cross_chunk_imports: Vec<ChunkIdx>,
  pub cross_chunk_dynamic_imports: Vec<ChunkIdx>,
  pub bits: BitSet,
  pub imports_from_other_chunks: Vec<(ChunkIdx, Vec<CrossChunkImportItem>)>,
  // Only meaningful for cjs format
  pub require_binding_names_for_other_chunks: FxHashMap<ChunkIdx, String>,
  pub imports_from_external_modules: Vec<(ModuleIdx, Vec<NamedImport>)>,
  // meaningless if the chunk is an entrypoint
  pub exports_to_other_chunks: FxHashMap<SymbolRef, Rstr>,
  pub is_alive: bool,
}

impl Chunk {
  pub fn new(
    name: Option<ArcStr>,
    bits: BitSet,
    modules: Vec<ModuleIdx>,
    kind: ChunkKind,
    is_alive: bool,
  ) -> Self {
    Self {
      exec_order: u32::MAX,
      modules,
      name: name.map(Into::into),
      bits,
      kind,
      is_alive,
      ..Self::default()
    }
  }

  pub fn has_side_effect(&self, runtime_id: ModuleIdx) -> bool {
    if self.modules.len() == 1 && self.modules[0] == runtime_id {
      return false;
    }
    true
  }

  pub fn import_path_for(&self, importee: &Chunk) -> String {
    let importee_filename = importee
      .absolute_preliminary_filename
      .as_ref()
      .expect("importee chunk should have absolute_preliminary_filename");
    let import_path = self.relative_path_for(importee_filename.as_path());
    if import_path.starts_with('.') {
      import_path
    } else {
      format!("./{import_path}")
    }
  }

  pub fn relative_path_for(&self, target: &Path) -> String {
    let sorurce_dir = self
      .absolute_preliminary_filename
      .as_ref()
      .expect("chunk should have absolute_preliminary_filename")
      .as_path()
      .parent()
      .expect("absolute_preliminary_filename should have a parent directory");
    target.relative(sorurce_dir).as_path().expect_to_slash()
  }

  pub fn filename_template(&self, options: &NormalizedBundlerOptions) -> FilenameTemplate {
    let ret = if matches!(self.kind, ChunkKind::EntryPoint { is_user_defined, .. } if is_user_defined)
    {
      options.entry_filenames.clone()
    } else {
      options.chunk_filenames.clone()
    };

    FilenameTemplate::new(ret)
  }

  pub fn css_filename_template(&self, options: &NormalizedBundlerOptions) -> FilenameTemplate {
    let ret = if matches!(self.kind, ChunkKind::EntryPoint { is_user_defined, .. } if is_user_defined)
    {
      options.css_entry_filenames.clone()
    } else {
      options.css_chunk_filenames.clone()
    };

    FilenameTemplate::new(ret)
  }

  pub fn generate_preliminary_filename(
    &mut self,
    options: &NormalizedBundlerOptions,
    chunk_name: &ArcStr,
    hash_placeholder_generator: &mut HashPlaceholderGenerator,
    make_unique_name: &mut impl FnMut(&ArcStr) -> ArcStr,
  ) -> anyhow::Result<PreliminaryFilename> {
    if let Some(file) = &options.file {
      return Ok(PreliminaryFilename::new(file.clone(), None));
    }

    let filename_template = self.filename_template(options);
    let has_hash_pattern = filename_template.has_hash_pattern();

    let name = if has_hash_pattern {
      make_unique_name(chunk_name);
      Cow::Borrowed(chunk_name)
    } else {
      let unique = make_unique_name(chunk_name);
      Cow::Owned(unique)
    };

    let mut hash_placeholder = has_hash_pattern.then_some(vec![]);
    let hash_replacer = has_hash_pattern.then_some({
      |len: Option<usize>| {
        let hash = hash_placeholder_generator.generate(len.unwrap_or(8));
        if let Some(hash_placeholder) = hash_placeholder.as_mut() {
          hash_placeholder.push(hash.clone());
        }
        hash
      }
    });

    let filename = filename_template.render(Some(&name), None, hash_replacer);

    Ok(PreliminaryFilename::new(filename, hash_placeholder))
  }

  pub fn generate_css_preliminary_filename(
    &self,
    options: &NormalizedBundlerOptions,
    chunk_name: &ArcStr,
    hash_placeholder_generator: &mut HashPlaceholderGenerator,
    make_unique_name: &mut impl FnMut(&ArcStr) -> ArcStr,
  ) -> anyhow::Result<PreliminaryFilename> {
    if let Some(file) = &options.file {
      let mut file = PathBuf::from(file);
      file.set_extension("css");
      return Ok(PreliminaryFilename::new(file.into_os_string().into_string().unwrap(), None));
    }

    let filename_template = self.css_filename_template(options);
    let has_hash_pattern = filename_template.has_hash_pattern();

    let name = if has_hash_pattern {
      make_unique_name(chunk_name);
      Cow::Borrowed(chunk_name)
    } else {
      let unique = make_unique_name(chunk_name);
      Cow::Owned(unique)
    };

    let mut hash_placeholder = has_hash_pattern.then_some(vec![]);
    let hash_replacer = has_hash_pattern.then_some({
      |len: Option<usize>| {
        let hash = hash_placeholder_generator.generate(len.unwrap_or(8));
        if let Some(hash_placeholder) = hash_placeholder.as_mut() {
          hash_placeholder.push(hash.clone());
        }
        hash
      }
    });

    let filename = filename_template.render(Some(&name), None, hash_replacer);

    Ok(PreliminaryFilename::new(filename, hash_placeholder))
  }

  pub fn user_defined_entry_module_idx(&self) -> Option<ModuleIdx> {
    match &self.kind {
      ChunkKind::EntryPoint { module, is_user_defined, .. } if *is_user_defined => Some(*module),
      _ => None,
    }
  }

  pub fn user_defined_entry_module<'module>(
    &self,
    modules: &'module IndexVec<ModuleIdx, Module>,
  ) -> Option<&'module NormalModule> {
    self.user_defined_entry_module_idx().and_then(|idx| modules[idx].as_normal())
  }

  pub fn entry_module_idx(&self) -> Option<ModuleIdx> {
    match &self.kind {
      ChunkKind::EntryPoint { module, .. } => Some(*module),
      ChunkKind::Common => None,
    }
  }

  pub fn entry_module<'module>(
    &self,
    modules: &'module IndexVec<ModuleIdx, Module>,
  ) -> Option<&'module NormalModule> {
    self.entry_module_idx().and_then(|idx| modules[idx].as_normal())
  }
}
