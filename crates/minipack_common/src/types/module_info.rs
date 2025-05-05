use arcstr::ArcStr;
use minipack_utils::indexmap::FxIndexSet;

use crate::ModuleId;

#[derive(Debug)]
pub struct ModuleInfo {
  pub id: ModuleId,
  pub is_entry: bool,
  pub code: Option<ArcStr>,
  pub importers: FxIndexSet<ModuleId>,
  pub imported_ids: FxIndexSet<ModuleId>,
  pub dynamic_importers: FxIndexSet<ModuleId>,
  pub dynamically_imported_ids: FxIndexSet<ModuleId>,
}
