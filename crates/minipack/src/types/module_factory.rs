use minipack_common::{ModuleIdx, ModuleType, ResolvedId};

pub struct CreateModuleContext<'a> {
  pub stable_id: &'a str,
  pub module_idx: ModuleIdx,
  pub resolved_id: &'a ResolvedId,
  pub module_type: ModuleType,
  pub warnings: &'a mut Vec<anyhow::Error>,
}
