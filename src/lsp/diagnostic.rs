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
    serde_json::json!({
        "range": {
            "start": { "line": err.line.saturating_sub(1), "character": err.col.saturating_sub(1) },
            "end": { "line": err.line.saturating_sub(1), "character": err.col }
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
