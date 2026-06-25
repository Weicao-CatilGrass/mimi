use serde_json::Value;

use crate::diagnostic::{Diagnostic, Severity};
use crate::lsp::position;
use crate::parser::ParseError;

pub(crate) fn severity_to_lsp(severity: &Severity) -> i32 {
    match severity {
        Severity::Error => 1,
        Severity::Warning => 2,
        Severity::Note => 3,
        Severity::Help => 4,
    }
}

pub(crate) fn diagnostic_to_lsp(diagnostic: &Diagnostic) -> Value {
    let code = diagnostic.code.clone().unwrap_or_default();
    serde_json::json!({
        "range": position::span_to_range(&diagnostic.span),
        "severity": severity_to_lsp(&diagnostic.severity),
        "source": "mimi",
        "code": code,
        "message": diagnostic.message
    })
}

pub(crate) fn parse_error_to_lsp(err: &ParseError) -> Value {
    // err.col is the column where the error occurred (1-indexed).
    // start: col-1 to get 0-indexed start position.
    // end: col to point just after the error token, but ensure it's at least col-1 + 1.
    let start_col = err.col.saturating_sub(1);
    let end_col = (err.col).max(start_col + 1);
    serde_json::json!({
        "range": {
            "start": { "line": err.line.saturating_sub(1), "character": start_col },
            "end": { "line": err.line.saturating_sub(1), "character": end_col }
        },
        "severity": 1,
        "source": "mimi",
        "message": err.message
    })
}

pub(crate) fn simple_error_diagnostic(message: &str) -> Value {
    serde_json::json!({
        "range": {
            "start": { "line": 0, "character": 0 },
            "end": { "line": 0, "character": 100 }
        },
        "severity": 1,
        "message": message
    })
}
