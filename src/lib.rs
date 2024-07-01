#![warn(clippy::pedantic, clippy::perf, missing_docs)]

//! Contains internal workings of the macro parser and runtime.

pub mod execution;
pub(crate) mod parsing;
pub mod stdlib;

pub use execution::{Macro, MacroError};
pub use stdlib::add_stdlib;
