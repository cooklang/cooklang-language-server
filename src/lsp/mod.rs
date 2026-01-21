//! LSP protocol conversion utilities.
//!
//! This module provides clean separation between LSP types and internal types,
//! following rust-analyzer's patterns.

pub mod from_proto;
pub mod line_endings;
pub mod to_proto;

pub use from_proto::*;
pub use line_endings::LineEndings;
pub use to_proto::*;
