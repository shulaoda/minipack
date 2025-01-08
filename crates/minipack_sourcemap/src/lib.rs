mod source;
mod source_joiner;

pub use crate::source::{Source, SourceMapSource};
pub use oxc_sourcemap::SourceMapBuilder;
pub use oxc_sourcemap::{JSONSourceMap, SourceMap, SourcemapVisualizer};
pub use source_joiner::SourceJoiner;
