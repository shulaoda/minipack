use minipack_common::{ModuleIdx, ModuleType, ResolvedId};

use super::SharedOptions;

pub struct CreateModuleContext<'a> {
  pub stable_id: &'a str,
  pub module_index: ModuleIdx,
  pub resolved_id: &'a ResolvedId,
  pub options: &'a SharedOptions,
  pub module_type: ModuleType,
  pub warnings: &'a mut Vec<anyhow::Error>,
  pub is_user_defined_entry: bool,
}
