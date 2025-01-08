mod file_system;
#[cfg(feature = "os")]
mod os;

pub use crate::file_system::FileSystem;
#[cfg(feature = "os")]
pub use os::OsFileSystem;
