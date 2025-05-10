pub mod external_module;
pub mod normal_module;

use oxc_index::IndexVec;

use crate::{
  EcmaAstIdx, ExternalModule, ImportRecordIdx, ModuleIdx, NormalModule, ResolvedImportRecord,
};

#[derive(Debug)]
pub enum Module {
  Normal(Box<NormalModule>),
  External(Box<ExternalModule>),
}

impl Module {
  pub fn idx(&self) -> ModuleIdx {
    match self {
      Self::Normal(v) => v.idx,
      Self::External(v) => v.idx,
    }
  }

  pub fn exec_order(&self) -> u32 {
    match self {
      Self::Normal(v) => v.exec_order,
      Self::External(v) => v.exec_order,
    }
  }

  pub fn id(&self) -> &str {
    match self {
      Self::Normal(v) => &v.id,
      Self::External(v) => &v.name,
    }
  }

  pub fn side_effects(&self) -> &crate::side_effects::DeterminedSideEffects {
    match self {
      Self::Normal(v) => &v.side_effects,
      Self::External(v) => &v.side_effects,
    }
  }

  pub fn stable_id(&self) -> &str {
    match self {
      Self::Normal(v) => &v.stable_id,
      Self::External(v) => &v.name,
    }
  }

  pub fn as_normal(&self) -> Option<&NormalModule> {
    match self {
      Self::Normal(v) => Some(v),
      Self::External(_) => None,
    }
  }

  pub fn as_external(&self) -> Option<&ExternalModule> {
    match self {
      Self::External(v) => Some(v),
      Self::Normal(_) => None,
    }
  }

  pub fn as_normal_mut(&mut self) -> Option<&mut NormalModule> {
    match self {
      Self::Normal(v) => Some(v),
      Self::External(_) => None,
    }
  }

  pub fn import_records(&self) -> &IndexVec<ImportRecordIdx, ResolvedImportRecord> {
    match self {
      Self::Normal(v) => &v.ecma_view.import_records,
      Self::External(v) => &v.import_records,
    }
  }

  pub fn set_import_records(&mut self, records: IndexVec<ImportRecordIdx, ResolvedImportRecord>) {
    match self {
      Self::Normal(v) => v.ecma_view.import_records = records,
      Self::External(v) => v.import_records = records,
    }
  }

  pub fn set_ecma_ast_idx(&mut self, idx: EcmaAstIdx) {
    match self {
      Self::Normal(v) => v.ecma_ast_idx = Some(idx),
      Self::External(_) => panic!("set_ecma_ast_idx should be called on EcmaModule"),
    }
  }

  pub fn is_normal(&self) -> bool {
    matches!(self, Self::Normal(..))
  }

  pub fn is_external(&self) -> bool {
    matches!(self, Self::External(..))
  }
}

impl From<NormalModule> for Module {
  fn from(module: NormalModule) -> Self {
    Self::Normal(Box::new(module))
  }
}

impl From<ExternalModule> for Module {
  fn from(module: ExternalModule) -> Self {
    Self::External(Box::new(module))
  }
}
