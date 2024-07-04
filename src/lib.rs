#![warn(clippy::pedantic, clippy::perf, missing_docs)]
#![allow(clippy::too_many_lines)]
// My editor highlights errors for these, even though I have Rust updated to 1.81.
#![feature(byte_slice_trim_ascii, lazy_cell)]
#![doc = include_str!("../README.md")]

pub mod execution;
pub(crate) mod parsing;
pub mod test;
pub mod stdlib;
pub mod textmacro;

pub use execution::{Macro, MacroError, MacroErrorKind, apply_macros};
pub use stdlib::add as add_stdlib;
pub use textmacro::TextMacro;
