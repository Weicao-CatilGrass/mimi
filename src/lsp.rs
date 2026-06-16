use crate::{core, lexer, parser};
use crate::ast::Item;
use std::collections::HashMap;
use std::io::{self, BufRead, Read, Write};

/// LSP server for Mimi language
pub struct LspServer {
    documents: HashMap<String, String>,
}

impl LspServer {
    pub fn new() -> Self {
        LspServer {
            documents: HashMap::new(),
        }
    }

    /// Run the LSP server (stdin/stdout JSON-RPC)
    pub fn run(&mut self) -> Result<(), String> {
        let stdin = io::stdin();
        let mut reader = stdin.lock();
        let mut buffer = String::new();

        loop {
            buffer.clear();
            // Read Content-Length header
            let mut header = String::new();
            loop {
                header.clear();
                if reader.read_line(&mut header).is_err() || header.is_empty() {
                    return Ok(());
                }
                if header.starts_with("Content-Length:") {
                    break;
                }
            }

            let len: usize = header.trim()
                .strip_prefix("Content-Length: ")
                .and_then(|s| s.parse().ok())
                .unwrap_or(0);

            if len == 0 {
                continue;
            }

            // Read JSON body
            let mut body = vec![0u8; len];
            reader.read_exact(&mut body).map_err(|e| format!("read error: {}", e))?;
            let body = String::from_utf8(body).map_err(|e| format!("utf8 error: {}", e))?;

            // Skip empty line after body
            let mut newline = [0u8; 1];
            let _ = io::stdin().read(&mut newline);

            // Parse and handle
            if let Ok(msg) = serde_json::from_str::<serde_json::Value>(&body) {
                if let Some(response) = self.handle_message(&msg) {
                    let resp_str = serde_json::to_string(&response).unwrap_or_default();
                    print!("Content-Length: {}\r\n\r\n{}", resp_str.len(), resp_str);
                    io::stdout().flush().ok();
                }
            }
        }
    }

    pub(crate) fn handle_message(&mut self, msg: &serde_json::Value) -> Option<serde_json::Value> {
        let method = msg.get("method")?.as_str()?;
        let id = msg.get("id");

        match method {
            "initialize" => {
                let result = serde_json::json!({
                    "capabilities": {
                        "textDocumentSync": 1,
                        "completionProvider": {
                            "triggerCharacters": [".", ":"]
                        },
                        "diagnosticProvider": {
                            "interFileDependencies": false,
                            "workspaceDiagnostics": false
                        }
                    }
                });
                Some(serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": result
                }))
            }
            "initialized" => None,
            "textDocument/didOpen" => {
                let uri = msg.get("params")?
                    .get("textDocument")?
                    .get("uri")?
                    .as_str()?;
                let text = msg.get("params")?
                    .get("textDocument")?
                    .get("text")?
                    .as_str()?;
                self.documents.insert(uri.to_string(), text.to_string());
                // Publish diagnostics
                let diagnostics = self.compute_diagnostics(text);
                Some(serde_json::json!({
                    "jsonrpc": "2.0",
                    "method": "textDocument/publishDiagnostics",
                    "params": {
                        "uri": uri,
                        "diagnostics": diagnostics
                    }
                }))
            }
            "textDocument/didChange" => {
                let uri = msg.get("params")?
                    .get("textDocument")?
                    .get("uri")?
                    .as_str()?;
                let text = msg.get("params")?
                    .get("contentChanges")?
                    .as_array()?
                    .first()?
                    .get("text")?
                    .as_str()?;
                self.documents.insert(uri.to_string(), text.to_string());
                let diagnostics = self.compute_diagnostics(text);
                Some(serde_json::json!({
                    "jsonrpc": "2.0",
                    "method": "textDocument/publishDiagnostics",
                    "params": {
                        "uri": uri,
                        "diagnostics": diagnostics
                    }
                }))
            }
            "textDocument/completion" => {
                let uri = msg.get("params")?
                    .get("textDocument")?
                    .get("uri")?
                    .as_str()?;
                let text = self.documents.get(uri)?;
                let items = self.compute_completion(text);
                Some(serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": {
                        "isIncomplete": false,
                        "items": items
                    }
                }))
            }
            "textDocument/hover" => {
                let uri = msg.get("params")?
                    .get("textDocument")?
                    .get("uri")?
                    .as_str()?;
                let position = msg.get("params")?
                    .get("position")?;
                let line = position.get("line")?.as_u64()? as usize;
                let character = position.get("character")?.as_u64()? as usize;
                let text = self.documents.get(uri)?;
                let hover = self.compute_hover(text, line, character);
                Some(serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": hover
                }))
            }
            "textDocument/definition" => {
                let uri = msg.get("params")?
                    .get("textDocument")?
                    .get("uri")?
                    .as_str()?;
                let position = msg.get("params")?
                    .get("position")?;
                let line = position.get("line")?.as_u64()? as usize;
                let character = position.get("character")?.as_u64()? as usize;
                let text = self.documents.get(uri)?;
                let definition = self.compute_definition(text, line, character, uri);
                Some(serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": definition
                }))
            }
            "textDocument/documentSymbol" => {
                let uri = msg.get("params")?
                    .get("textDocument")?
                    .get("uri")?
                    .as_str()?;
                let text = self.documents.get(uri)?;
                let symbols = self.compute_document_symbols(text);
                Some(serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": symbols
                }))
            }
            "shutdown" => {
                Some(serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": null
                }))
            }
            "exit" => std::process::exit(0),
            _ => None,
        }
    }

    pub fn compute_diagnostics(&self, text: &str) -> Vec<serde_json::Value> {
        let mut diagnostics = Vec::new();

        // Parse
        let tokens = match lexer::Lexer::new(text).tokenize() {
            Ok(t) => t,
            Err(e) => {
                diagnostics.push(serde_json::json!({
                    "range": {
                        "start": { "line": 0, "character": 0 },
                        "end": { "line": 0, "character": 100 }
                    },
                    "severity": 1,
                    "message": e
                }));
                return diagnostics;
            }
        };

        let file = match parser::Parser::new(tokens).parse_file() {
            Ok(f) => f,
            Err(e) => {
                diagnostics.push(serde_json::json!({
                    "range": {
                        "start": { "line": e.line.saturating_sub(1), "character": e.col.saturating_sub(1) },
                        "end": { "line": e.line.saturating_sub(1), "character": e.col }
                    },
                    "severity": 1,
                    "message": e.message
                }));
                return diagnostics;
            }
        };

        // Type check
        if let Err(errs) = core::check(&file) {
            for err in errs {
                diagnostics.push(serde_json::json!({
                    "range": {
                        "start": { "line": err.span.start_line.saturating_sub(1), "character": err.span.start_col.saturating_sub(1) },
                        "end": { "line": err.span.end_line.saturating_sub(1), "character": err.span.end_col.saturating_sub(1) }
                    },
                    "severity": 1,
                    "source": "mimi",
                    "message": err.message
                }));
            }
        }

        diagnostics
    }

    pub fn compute_document_symbols(&self, text: &str) -> Vec<serde_json::Value> {
        let mut symbols = Vec::new();

        if let Ok(tokens) = lexer::Lexer::new(text).tokenize() {
            if let Ok(file) = parser::Parser::new(tokens).parse_file() {
                for item in &file.items {
                    match item {
                        Item::Func(f) => {
                            // Find the line where the function is defined
                            let def_line = text.lines().position(|l| l.contains(&format!("func {}", f.name))).unwrap_or(0);
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
                            let def_line = text.lines().position(|l| l.contains(&format!("type {}", t.name))).unwrap_or(0);
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
                            let def_line = text.lines().position(|l| l.contains(&format!("module {}", m.name))).unwrap_or(0);
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
        }

        symbols
    }

    pub fn compute_definition(&self, text: &str, line: usize, character: usize, uri: &str) -> Option<serde_json::Value> {
        // Get the word at cursor position
        let lines: Vec<&str> = text.lines().collect();
        let current_line = lines.get(line)?;
        let before_cursor: String = current_line.chars().take(character).collect();
        let after_cursor: String = current_line.chars().skip(character).collect();

        // Find word boundaries
        let word_start = before_cursor.rfind(|c: char| !c.is_alphanumeric() && c != '_').map(|i| i + 1).unwrap_or(0);
        let word_end = after_cursor.find(|c: char| !c.is_alphanumeric() && c != '_').map(|i| character + i).unwrap_or(current_line.len());
        let word = &current_line[word_start..word_end];

        if word.is_empty() {
            return None;
        }

        // Try to parse and find the symbol definition
        if let Ok(tokens) = lexer::Lexer::new(text).tokenize() {
            if let Ok(file) = parser::Parser::new(tokens).parse_file() {
                for item in &file.items {
                    match item {
                        Item::Func(f) if f.name == word => {
                            // Find the line where the function is defined
                            let def_line = text.lines().position(|l| l.contains(&format!("func {}", word))).unwrap_or(0);
                            return Some(serde_json::json!({
                                "uri": uri,
                                "range": {
                                    "start": { "line": def_line, "character": 0 },
                                    "end": { "line": def_line, "character": 100 }
                                }
                            }));
                        }
                        Item::Type(t) if t.name == word => {
                            let def_line = text.lines().position(|l| l.contains(&format!("type {}", word))).unwrap_or(0);
                            return Some(serde_json::json!({
                                "uri": uri,
                                "range": {
                                    "start": { "line": def_line, "character": 0 },
                                    "end": { "line": def_line, "character": 100 }
                                }
                            }));
                        }
                        Item::Module(m) if m.name == word => {
                            let def_line = text.lines().position(|l| l.contains(&format!("module {}", word))).unwrap_or(0);
                            return Some(serde_json::json!({
                                "uri": uri,
                                "range": {
                                    "start": { "line": def_line, "character": 0 },
                                    "end": { "line": def_line, "character": 100 }
                                }
                            }));
                        }
                        _ => {}
                    }
                }
            }
        }

        // Builtins don't have definitions in user code
        None
    }

    pub fn compute_hover(&self, text: &str, line: usize, character: usize) -> Option<serde_json::Value> {
        // Get the word at cursor position
        let lines: Vec<&str> = text.lines().collect();
        let current_line = lines.get(line)?;
        let before_cursor: String = current_line.chars().take(character).collect();
        let after_cursor: String = current_line.chars().skip(character).collect();

        // Find word boundaries
        let word_start = before_cursor.rfind(|c: char| !c.is_alphanumeric() && c != '_').map(|i| i + 1).unwrap_or(0);
        let word_end = after_cursor.find(|c: char| !c.is_alphanumeric() && c != '_').map(|i| character + i).unwrap_or(current_line.len());
        let word = &current_line[word_start..word_end];

        if word.is_empty() {
            return None;
        }

        // Try to parse and find the symbol
        if let Ok(tokens) = lexer::Lexer::new(text).tokenize() {
            if let Ok(file) = parser::Parser::new(tokens).parse_file() {
                for item in &file.items {
                    match item {
                        Item::Func(f) if f.name == word => {
                            let params: Vec<String> = f.params.iter()
                                .map(|p| format!("{}: {:?}", p.name, p.ty))
                                .collect();
                            let ret = f.ret.as_ref().map(|t| format!(" -> {:?}", t)).unwrap_or_default();
                            return Some(serde_json::json!({
                                "contents": {
                                    "kind": "markdown",
                                    "value": format!("**func** `{}({}){}`", word, params.join(", "), ret)
                                }
                            }));
                        }
                        Item::Type(t) if t.name == word => {
                            return Some(serde_json::json!({
                                "contents": {
                                    "kind": "markdown",
                                    "value": format!("**type** `{}`", word)
                                }
                            }));
                        }
                        Item::Module(m) if m.name == word => {
                            return Some(serde_json::json!({
                                "contents": {
                                    "kind": "markdown",
                                    "value": format!("**module** `{}`", word)
                                }
                            }));
                        }
                        _ => {}
                    }
                }
            }
        }

        // Check builtins
        let builtins = vec![
            ("println", "fn println(args...)"),
            ("assert", "fn assert(condition: bool)"),
            ("assert_eq", "fn assert_eq(a, b)"),
            ("len", "fn len(collection) -> i64"),
            ("push", "fn push(list, item)"),
            ("pop", "fn pop(list) -> item"),
            ("range", "fn range(n) -> list"),
            ("sqrt", "fn sqrt(x: f64) -> f64"),
            ("abs", "fn abs(x) -> x"),
            ("min", "fn min(a, b) -> a"),
            ("max", "fn max(a, b) -> a"),
            ("to_string", "fn to_string(val) -> string"),
            ("print", "fn print(args...)"),
            ("pow", "fn pow(base, exp) -> result"),
            ("floor", "fn floor(x: f64) -> i64"),
            ("ceil", "fn ceil(x: f64) -> i64"),
            ("round", "fn round(x: f64) -> i64"),
            ("random", "fn random() -> f64"),
            ("pi", "fn pi() -> f64"),
            ("read_file", "fn read_file(path: string) -> string"),
            ("write_file", "fn write_file(path: string, content: string)"),
            ("file_exists", "fn file_exists(path: string) -> bool"),
            ("to_int", "fn to_int(val) -> i64"),
            ("to_float", "fn to_float(val) -> f64"),
            ("str_char_at", "fn str_char_at(s: string, i: i64) -> string"),
            ("str_substring", "fn str_substring(s: string, start: i64, len: i64) -> string"),
            ("str_parse_int", "fn str_parse_int(s: string) -> (bool, i64)"),
            ("str_parse_float", "fn str_parse_float(s: string) -> (bool, f64)"),
            ("keys", "fn keys(record) -> list"),
            ("values", "fn values(record) -> list"),
            ("has_key", "fn has_key(record, key) -> bool"),
        ];

        for (name, sig) in builtins {
            if word == name {
                return Some(serde_json::json!({
                    "contents": {
                        "kind": "markdown",
                        "value": format!("**builtin** `{}`", sig)
                    }
                }));
            }
        }

        None
    }

    pub fn compute_completion(&self, text: &str) -> Vec<serde_json::Value> {
        let mut items = Vec::new();

        // Keywords
        let keywords = vec![
            "func", "type", "flow", "module", "if", "else", "while", "for",
            "return", "let", "mut", "shared", "local_shared", "weak",
            "match", "spawn", "await", "try", "comptime", "quote",
            "extern", "actor", "trait", "impl", "cap", "true", "false",
        ];

        for kw in keywords {
            items.push(serde_json::json!({
                "label": kw,
                "kind": 14, // Keyword
                "insertText": kw,
            }));
        }

        // Try to parse and extract function names
        if let Ok(tokens) = lexer::Lexer::new(text).tokenize() {
            if let Ok(file) = parser::Parser::new(tokens).parse_file() {
                for item in &file.items {
                    match item {
                        Item::Func(f) => {
                            items.push(serde_json::json!({
                                "label": f.name,
                                "kind": 3, // Function
                                "detail": format!("func {}(...)", f.name),
                                "insertText": format!("{}(${{1}})", f.name),
                                "insertTextFormat": 2, // Snippet
                            }));
                        }
                        Item::Type(t) => {
                            items.push(serde_json::json!({
                                "label": t.name,
                                "kind": 22, // TypeParameter
                                "detail": format!("type {}", t.name),
                            }));
                        }
                        Item::Module(m) => {
                            items.push(serde_json::json!({
                                "label": m.name,
                                "kind": 1, // Module
                                "detail": format!("module {}", m.name),
                            }));
                        }
                        _ => {}
                    }
                }
            }
        }

        // Builtins (updated with v5.0 additions)
        let builtins = vec![
            "println", "print", "assert", "assert_eq", "assert_ne", "len", "push",
            "pop", "range", "sqrt", "abs", "min", "max", "to_string",
            "map", "filter", "reduce", "sort", "reverse", "flatten",
            "zip", "enumerate", "sum", "contains", "input",
            "type_name", "type_fields", "type_variants", "type_info",
            "ast_dump", "ast_eval",
            // v5.0 additions
            "pow", "floor", "ceil", "round", "random", "pi",
            "read_file", "write_file", "file_exists",
            "to_int", "to_float",
            "str_char_at", "str_substring", "str_parse_int", "str_parse_float",
            "keys", "values", "has_key",
        ];

        for b in builtins {
            items.push(serde_json::json!({
                "label": b,
                "kind": 12, // Function (builtin)
                "detail": format!("builtin {}", b),
                "insertText": format!("{}(${{1}})", b),
                "insertTextFormat": 2,
            }));
        }

        items
    }
}
