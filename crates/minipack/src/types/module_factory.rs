use minipack_common::{
  side_effects::HookSideEffects, ModuleIdx, ModuleType, ResolvedId, StrOrBytes,
};

use super::SharedOptions;

pub struct CreateModuleContext<'a> {
  pub module_index: ModuleIdx,
  pub resolved_id: &'a ResolvedId,
  pub options: &'a SharedOptions,
  pub module_type: ModuleType,
  pub warnings: &'a mut Vec<anyhow::Error>,
  pub is_user_defined_entry: bool,
}

pub struct CreateModuleViewArgs {
  pub source: StrOrBytes,
  pub hook_side_effects: Option<HookSideEffects>,
}
