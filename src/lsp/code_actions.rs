use serde_json::Value;

use crate::lsp::LspServer;

impl LspServer {
    /// Compute code actions (quick fixes) for the given diagnostics context
    pub fn compute_code_actions(&self, uri: &str, context: &Value) -> Vec<Value> {
        let mut actions = Vec::new();
        let Some(diagnostics) = context.get("diagnostics").and_then(|d| d.as_array()) else {
            return actions;
        };
        for diag in diagnostics {
            let Some(code) = diag.get("code").and_then(|c| c.as_str()) else {
                continue;
            };
            let Some(msg) = diag.get("message").and_then(|m| m.as_str()) else {
                continue;
            };
            let _range = diag.get("range").cloned().unwrap_or_default();
            match code {
                crate::diagnostic::codes::E0400 => {
                    if let Some(name) = extract_quoted_name(msg) {
                        let edit = serde_json::json!({
                            "changes": {
                                uri: [
                                    {
                                        "range": { "start": { "line": 0, "character": 0 }, "end": { "line": 0, "character": 0 } },
                                        "newText": format!("let {} = \n", name)
                                    }
                                ]
                            }
                        });
                        actions.push(serde_json::json!({
                            "title": format!("Create variable `{}`", name),
                            "kind": "quickfix",
                            "diagnostics": [diag.clone()],
                            "edit": edit
                        }));
                    }
                }
                crate::diagnostic::codes::E0401 => {
                    if let Some(name) = extract_quoted_name(msg) {
                        let edit = serde_json::json!({
                            "changes": {
                                uri: [
                                    {
                                        "range": { "start": { "line": 0, "character": 0 }, "end": { "line": 0, "character": 0 } },
                                        "newText": format!("func {}() -> i32 {{\n    \n}}\n", name)
                                    }
                                ]
                            }
                        });
                        actions.push(serde_json::json!({
                            "title": format!("Create function `{}`", name),
                            "kind": "quickfix",
                            "diagnostics": [diag.clone()],
                            "edit": edit
                        }));
                    }
                }
                crate::diagnostic::codes::E0406 => {
                    if let Some(name) = extract_quoted_name(msg) {
                        let edit = serde_json::json!({
                            "changes": {
                                uri: [
                                    {
                                        "range": { "start": { "line": 0, "character": 0 }, "end": { "line": 0, "character": 0 } },
                                        "newText": format!("trait {} {{\n    \n}}\n", name)
                                    }
                                ]
                            }
                        });
                        actions.push(serde_json::json!({
                            "title": format!("Create trait `{}`", name),
                            "kind": "quickfix",
                            "diagnostics": [diag.clone()],
                            "edit": edit
                        }));
                    }
                }
                crate::diagnostic::codes::E0231 | crate::diagnostic::codes::E0407 => {
                    if let Some(name) = extract_quoted_name(msg) {
                        let edit = serde_json::json!({
                            "changes": {
                                uri: [
                                    {
                                        "range": { "start": { "line": 0, "character": 0 }, "end": { "line": 0, "character": 0 } },
                                        "newText": format!("type {} = i64\n", name)
                                    }
                                ]
                            }
                        });
                        actions.push(serde_json::json!({
                            "title": format!("Create type alias `{}`", name),
                            "kind": "quickfix",
                            "diagnostics": [diag.clone()],
                            "edit": edit
                        }));
                    }
                }
                _ => {}
            }
        }
        actions
    }
}

/// Extract a name between single quotes from a diagnostic message
pub(crate) fn extract_quoted_name(msg: &str) -> Option<String> {
    let start = msg.find('\'')?;
    let rest = &msg[start + 1..];
    let end = rest.find('\'')?;
    Some(rest[..end].to_string())
}
