// cSpell:disable
pub use oxc_sourcemap::SourceMapBuilder;
pub use oxc_sourcemap::{JSONSourceMap, SourceMap, SourcemapVisualizer};
pub use source_joiner::SourceJoiner;
mod lines_count;
pub use lines_count::lines_count;
mod source;
mod source_joiner;

pub use crate::source::{Source, SourceMapSource};
