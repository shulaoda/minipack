// An wrapper around the `oxc_resolver` crate to provide a more rolldown-specific API.

pub mod error;
mod resolver;

pub use crate::resolver::{ResolveReturn, Resolver};

pub use minipack_common::ResolveOptions;
pub use oxc_resolver::ResolveError;
