use serde::Serialize;
use std::fmt;

/// Source location for diagnostic reporting
#[derive(Debug, Clone, Serialize)]
pub struct Span {
    pub start: usize,
    pub end: usize,
    pub line: usize,
    pub col: usize,
}

pub type Result<T> = std::result::Result<T, IroncladError>;

#[derive(Debug)]
pub enum IroncladError {
    ParseError { message: String, span: Option<Span> },
    ValidationError { errors: Vec<Diagnostic> },
}

impl fmt::Display for IroncladError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            IroncladError::ParseError { message, span } => {
                if let Some(s) = span {
                    write!(f, "parse error at {}:{}: {}", s.line, s.col, message)
                } else {
                    write!(f, "parse error: {message}")
                }
            }
            IroncladError::ValidationError { errors } => {
                for (i, e) in errors.iter().enumerate() {
                    if i > 0 {
                        writeln!(f)?;
                    }
                    write!(f, "{e}")?;
                }
                Ok(())
            }
        }
    }
}

impl std::error::Error for IroncladError {}

#[derive(Debug, Clone)]
pub struct Diagnostic {
    pub severity: Severity,
    pub message: String,
    pub span: Option<Span>,
    pub hint: Option<String>,
    pub block_name: Option<String>,
}

impl fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let prefix = match self.severity {
            Severity::Error => "error",
            Severity::Warning => "warning",
        };

        if let Some(ref span) = self.span {
            write!(f, "{prefix} [{}:{}]", span.line, span.col)?;
        } else {
            write!(f, "{prefix}")?;
        }

        if let Some(ref name) = self.block_name {
            write!(f, " in `{name}`")?;
        }

        write!(f, ": {}", self.message)?;

        if let Some(ref hint) = self.hint {
            write!(f, "\n  hint: {hint}")?;
        }

        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Error,
    Warning,
}

/// Format diagnostics with source context for pretty-printing
pub fn format_diagnostic(diag: &Diagnostic, source: &str) -> String {
    let mut out = String::new();

    let severity_str = match diag.severity {
        Severity::Error => "\x1b[1;31merror\x1b[0m",
        Severity::Warning => "\x1b[1;33mwarning\x1b[0m",
    };

    if let Some(ref name) = diag.block_name {
        out.push_str(&format!("{severity_str} in `{name}`: {}\n", diag.message));
    } else {
        out.push_str(&format!("{severity_str}: {}\n", diag.message));
    }

    if let Some(ref span) = diag.span {
        let lines: Vec<&str> = source.lines().collect();
        let line_idx = span.line.saturating_sub(1);
        let line_num = span.line;

        // Show the source line
        if line_idx < lines.len() {
            let gutter = format!("{line_num}");
            let pad: String = " ".repeat(gutter.len());

            out.push_str(&format!("{pad} \x1b[1;34m|\x1b[0m\n"));
            out.push_str(&format!(
                "\x1b[1;34m{gutter} |\x1b[0m {}\n",
                lines[line_idx]
            ));

            // Underline
            let col = span.col.saturating_sub(1);
            let underline_len = (span.end - span.start)
                .max(1)
                .min(lines[line_idx].len().saturating_sub(col));
            out.push_str(&format!(
                "{pad} \x1b[1;34m|\x1b[0m {}\x1b[1;31m{}\x1b[0m\n",
                " ".repeat(col),
                "^".repeat(underline_len.max(1))
            ));
        }
    }

    if let Some(ref hint) = diag.hint {
        out.push_str(&format!("  \x1b[1;36mhint\x1b[0m: {hint}\n"));
    }

    out
}
