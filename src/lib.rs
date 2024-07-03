#![warn(clippy::pedantic, clippy::perf, missing_docs)]

#![doc = include_str!("../README.md")]

pub mod execution;
pub(crate) mod parsing;
pub mod test;
pub mod stdlib;
pub mod textmacro;

pub use execution::{Macro, MacroError, MacroErrorKind, apply_macros};
pub use stdlib::add as add_stdlib;
pub use textmacro::TextMacro;
