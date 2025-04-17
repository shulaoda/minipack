use arcstr::ArcStr;
use rustc_hash::FxHashMap;

use crate::ModuleId;

use super::rendered_module::RenderedModule;

#[derive(Debug, Clone)]
pub struct OutputChunk {

  // RenderedChunk
  pub filename: ArcStr,

  // OutputChunk
  pub code: String,
  pub preliminary_filename: String,
}

#[derive(Debug, Clone)]
pub struct Modules {
  pub keys: Vec<ModuleId>,
  pub values: Vec<RenderedModule>,
}

impl From<FxHashMap<ModuleId, RenderedModule>> for Modules {
  fn from(value: FxHashMap<ModuleId, RenderedModule>) -> Self {
    let mut kvs = value.into_iter().collect::<Vec<_>>();
    kvs.sort_by(|a, b| a.1.exec_order.cmp(&b.1.exec_order));

    let mut keys = Vec::with_capacity(kvs.len());
    let mut values = Vec::with_capacity(kvs.len());
    for (k, v) in kvs {
      keys.push(k);
      values.push(v);
    }

    Self { keys, values }
  }
}
