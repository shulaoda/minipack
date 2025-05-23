use runtime_task_result::RuntimeModuleTaskResult;
use task_result::NormalModuleTaskResult;

pub mod runtime_module_brief;
pub mod runtime_task_result;
pub mod task_result;

pub enum ModuleLoaderMsg {
  RuntimeModuleDone(Box<RuntimeModuleTaskResult>),
  NormalModuleDone(Box<NormalModuleTaskResult>),
  BuildErrors(Vec<anyhow::Error>),
}
