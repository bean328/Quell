//! What a diagnostic says. Rendering lives in `render.rs`.

use crate::source::Span;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Severity {
    Error,
    Warning,
    Note,
}

impl Severity {
    pub fn as_str(self) -> &'static str {
        match self {
            Severity::Error => "error",
            Severity::Warning => "warning",
            Severity::Note => "note",
        }
    }
}

/// A highlighted piece of source code. The primary label marks where the problem is,
/// secondary labels give context ("declared here").
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Label {
    pub span: Span,
    pub message: String,
    pub primary: bool,
}

/// Replace the text in `span` with `replacement`. An empty span inserts.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Edit {
    pub span: Span,
    pub replacement: String,
}

/// Advice on how to fix the problem, optionally with the exact edits that fix it.
/// Editors apply the edits as a quick fix; the terminal shows the fixed lines.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Help {
    pub message: String,
    pub edits: Vec<Edit>,
}

/// One message from the compiler. Every Quell error explains what happened,
/// shows where, and says how to fix it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Diagnostic {
    pub severity: Severity,
    /// A stable code like `Q0207`, explained in full by `quell explain Q0207`.
    pub code: Option<&'static str>,
    pub message: String,
    pub labels: Vec<Label>,
    pub notes: Vec<String>,
    pub helps: Vec<Help>,
}

impl Diagnostic {
    pub fn new(severity: Severity, message: impl Into<String>) -> Diagnostic {
        Diagnostic {
            severity,
            code: None,
            message: message.into(),
            labels: Vec::new(),
            notes: Vec::new(),
            helps: Vec::new(),
        }
    }

    pub fn error(message: impl Into<String>) -> Diagnostic {
        Diagnostic::new(Severity::Error, message)
    }

    pub fn warning(message: impl Into<String>) -> Diagnostic {
        Diagnostic::new(Severity::Warning, message)
    }

    pub fn with_code(mut self, code: &'static str) -> Diagnostic {
        self.code = Some(code);
        self
    }

    /// Marks where the problem is.
    pub fn primary(mut self, span: Span, message: impl Into<String>) -> Diagnostic {
        self.labels.push(Label { span, message: message.into(), primary: true });
        self
    }

    /// Marks a related place, like where something was declared.
    pub fn secondary(mut self, span: Span, message: impl Into<String>) -> Diagnostic {
        self.labels.push(Label { span, message: message.into(), primary: false });
        self
    }

    pub fn note(mut self, message: impl Into<String>) -> Diagnostic {
        self.notes.push(message.into());
        self
    }

    pub fn help(mut self, message: impl Into<String>) -> Diagnostic {
        self.helps.push(Help { message: message.into(), edits: Vec::new() });
        self
    }

    /// Advice with a single edit that fixes the problem.
    pub fn suggest(
        self,
        message: impl Into<String>,
        span: Span,
        replacement: impl Into<String>,
    ) -> Diagnostic {
        self.suggest_edits(message, vec![Edit { span, replacement: replacement.into() }])
    }

    /// Advice with several edits that together fix the problem.
    pub fn suggest_edits(mut self, message: impl Into<String>, edits: Vec<Edit>) -> Diagnostic {
        self.helps.push(Help { message: message.into(), edits });
        self
    }

    pub fn is_error(&self) -> bool {
        self.severity == Severity::Error
    }

    /// The label that decides the `-->` location: the first primary label, or the first label.
    pub fn main_label(&self) -> Option<&Label> {
        self.labels.iter().find(|label| label.primary).or_else(|| self.labels.first())
    }
}
