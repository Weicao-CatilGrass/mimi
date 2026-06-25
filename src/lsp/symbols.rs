use serde_json::Value;

use crate::ast::{Item, TypeDefKind};
use crate::lsp::LspServer;

impl LspServer {
    pub fn compute_document_symbols(&self, text: &str) -> Vec<Value> {
        let mut symbols = Vec::new();

        if let Some(file) = self.parse_with_recovery(text) {
            for item in &file.items {
                match item {
                    Item::Func(f) => {
                        // Find the line where the function is defined
                        let def_line = text
                            .lines()
                            .position(|l| l.contains(&format!("func {}", f.name)))
                            .unwrap_or(0);
                        symbols.push(serde_json::json!({
                            "name": f.name,
                            "kind": 12, // Function
                            "range": {
                                "start": { "line": def_line, "character": 0 },
                                "end": { "line": def_line, "character": 100 }
                            },
                            "selectionRange": {
                                "start": { "line": def_line, "character": 5 },
                                "end": { "line": def_line, "character": 5 + f.name.len() }
                            }
                        }));
                    }
                    Item::Type(t) => {
                        let def_line = text
                            .lines()
                            .position(|l| l.contains(&format!("type {}", t.name)))
                            .unwrap_or(0);
                        symbols.push(serde_json::json!({
                            "name": t.name,
                            "kind": 26, // Enum
                            "range": {
                                "start": { "line": def_line, "character": 0 },
                                "end": { "line": def_line, "character": 100 }
                            },
                            "selectionRange": {
                                "start": { "line": def_line, "character": 5 },
                                "end": { "line": def_line, "character": 5 + t.name.len() }
                            }
                        }));
                    }
                    Item::Module(m) => {
                        let def_line = text
                            .lines()
                            .position(|l| l.contains(&format!("module {}", m.name)))
                            .unwrap_or(0);
                        symbols.push(serde_json::json!({
                            "name": m.name,
                            "kind": 1, // Module
                            "range": {
                                "start": { "line": def_line, "character": 0 },
                                "end": { "line": def_line, "character": 100 }
                            },
                            "selectionRange": {
                                "start": { "line": def_line, "character": 7 },
                                "end": { "line": def_line, "character": 7 + m.name.len() }
                            }
                        }));
                    }
                    _ => {}
                }
            }
        }

        symbols
    }

    /// Compute workspace symbols (across all known .mimi files)
    pub fn compute_workspace_symbols(&self, query: &str) -> Vec<Value> {
        let mut symbols = Vec::new();
        let query_lower = query.to_lowercase();

        let mut sources: Vec<(String, String)> = self
            .documents
            .iter()
            .map(|(uri, text)| (uri.clone(), text.clone()))
            .collect();

        if let Some(root) = &self.workspace_root {
            if let Ok(entries) = std::fs::read_dir(root) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.extension().and_then(|e| e.to_str()) == Some("mimi") {
                        let uri = format!("file://{}", path.display());
                        if !self.documents.contains_key(&uri) {
                            if let Ok(text) = std::fs::read_to_string(&path) {
                                sources.push((uri, text));
                            }
                        }
                    }
                }
            }
        }

        for (uri, text) in &sources {
            let file = match self.parse_with_recovery(text) {
                Some(f) => f,
                None => continue,
            };
            for item in &file.items {
                match item {
                    Item::Func(f) => {
                        if !query_lower.is_empty() && !f.name.to_lowercase().contains(&query_lower)
                        {
                            continue;
                        }
                        let def_line = text
                            .lines()
                            .position(|l| l.contains(&format!("func {}", f.name)))
                            .unwrap_or(0);
                        symbols.push(ws_symbol(&f.name, 12, uri, def_line, ""));
                    }
                    Item::Type(t) => {
                        if !query_lower.is_empty() && !t.name.to_lowercase().contains(&query_lower)
                        {
                            continue;
                        }
                        let def_line = text
                            .lines()
                            .position(|l| l.contains(&format!("type {}", t.name)))
                            .unwrap_or(0);
                        let kind = match &t.kind {
                            TypeDefKind::Record(_) => 23,
                            TypeDefKind::Enum(_) => 10,
                            TypeDefKind::Union(_) => 24,
                            _ => 4,
                        };
                        symbols.push(ws_symbol(&t.name, kind, uri, def_line, ""));
                        if let TypeDefKind::Enum(variants) = &t.kind {
                            for variant in variants {
                                if !query_lower.is_empty()
                                    && !variant.name.to_lowercase().contains(&query_lower)
                                {
                                    continue;
                                }
                                let v_line = text
                                    .lines()
                                    .position(|l| l.contains(&variant.name))
                                    .unwrap_or(def_line);
                                symbols.push(ws_symbol(
                                    &format!("{}::{}", t.name, variant.name),
                                    23,
                                    uri,
                                    v_line,
                                    &t.name,
                                ));
                            }
                        }
                    }
                    Item::Trait(t) => {
                        if !query_lower.is_empty() && !t.name.to_lowercase().contains(&query_lower)
                        {
                            continue;
                        }
                        let def_line = text
                            .lines()
                            .position(|l| l.contains(&format!("trait {}", t.name)))
                            .unwrap_or(0);
                        symbols.push(ws_symbol(&t.name, 17, uri, def_line, ""));
                    }
                    Item::Impl(i) => {
                        if !query_lower.is_empty()
                            && !i.type_name.to_lowercase().contains(&query_lower)
                        {
                            continue;
                        }
                        let def_line = text.lines().position(|l| l.contains("impl")).unwrap_or(0);
                        symbols.push(ws_symbol(&i.type_name, 25, uri, def_line, &i.trait_name));
                    }
                    Item::Actor(a) => {
                        if !query_lower.is_empty() && !a.name.to_lowercase().contains(&query_lower)
                        {
                            continue;
                        }
                        let def_line = text
                            .lines()
                            .position(|l| l.contains(&format!("actor {}", a.name)))
                            .unwrap_or(0);
                        symbols.push(ws_symbol(&a.name, 23, uri, def_line, ""));
                    }
                    Item::Module(m) => {
                        if !query_lower.is_empty() && !m.name.to_lowercase().contains(&query_lower)
                        {
                            continue;
                        }
                        let def_line = text
                            .lines()
                            .position(|l| l.contains(&format!("module {}", m.name)))
                            .unwrap_or(0);
                        symbols.push(ws_symbol(&m.name, 2, uri, def_line, ""));
                    }
                    _ => {}
                }
            }
        }
        symbols
    }

    /// Prepare call hierarchy: find the function at the given position
    pub fn compute_prepare_call_hierarchy(
        &self,
        text: &str,
        uri: &str,
        line: usize,
        character: usize,
    ) -> Vec<Value> {
        let file = match self.parse_with_recovery(text) {
            Some(f) => f,
            None => return vec![],
        };
        let word = self.get_word_at(text, line, character);
        if word.is_empty() {
            return vec![];
        }
        for item in &file.items {
            match item {
                Item::Func(f) if f.name == word => {
                    let def_line = text
                        .lines()
                        .position(|l| l.contains(&format!("func {}", f.name)))
                        .unwrap_or(0);
                    return vec![serde_json::json!({
                        "name": f.name,
                        "kind": 12,
                        "uri": uri,
                        "range": {
                            "start": { "line": def_line, "character": 0 },
                            "end": { "line": def_line, "character": 0 }
                        },
                        "selectionRange": {
                            "start": { "line": def_line, "character": 5 },
                            "end": { "line": def_line, "character": 5 + f.name.len() }
                        }
                    })];
                }
                Item::Type(t) if t.name == word => {
                    let def_line = text
                        .lines()
                        .position(|l| l.contains(&format!("type {}", t.name)))
                        .unwrap_or(0);
                    return vec![serde_json::json!({
                        "name": t.name,
                        "kind": match t.kind {
                            TypeDefKind::Record(_) => 23,
                            TypeDefKind::Enum(_) => 10,
                            _ => 4
                        },
                        "uri": uri,
                        "range": {
                            "start": { "line": def_line, "character": 0 },
                            "end": { "line": def_line, "character": 0 }
                        },
                        "selectionRange": {
                            "start": { "line": def_line, "character": 5 },
                            "end": { "line": def_line, "character": 5 + t.name.len() }
                        }
                    })];
                }
                _ => {}
            }
        }
        vec![]
    }
}

/// Build a workspace symbol JSON object
pub(crate) fn ws_symbol(name: &str, kind: u32, uri: &str, line: usize, container: &str) -> Value {
    let mut obj = serde_json::json!({
        "name": name,
        "kind": kind,
        "location": {
            "uri": uri,
            "range": {
                "start": { "line": line, "character": 0 },
                "end": { "line": line, "character": 0 }
            }
        }
    });
    if !container.is_empty() {
        obj["containerName"] = serde_json::Value::String(container.to_string());
    }
    obj
}

/// Count how many times a name appears in text (simple substring match on each line)
pub(crate) fn count_text_references(text: &str, name: &str) -> usize {
    text.lines().filter(|l| l.contains(name)).count()
}
