//! Source files, spans and error messages for the Quell compiler.
//!
//! Every stage of the compiler reports problems as [`Diagnostic`]s. A diagnostic
//! says what went wrong, points at the code with labels, and offers help, ideally
//! with the exact [`Edit`]s that fix the problem.

mod diagnostic;
mod render;
mod source;

pub use diagnostic::{Diagnostic, Edit, Help, Label, Severity};
pub use render::{RenderOptions, render, render_all};
pub use source::{LineCol, SourceFile, Span};
