use std::collections::{HashMap, VecDeque};
use std::io::{self, BufRead, Read, Write};
use std::path::PathBuf;

use crate::verifier::{VerifStatus, Verifier};

pub(crate) mod code_actions;
pub(crate) mod completion;
pub(crate) mod diagnostic;
pub(crate) mod folding;
pub(crate) mod handlers;
pub(crate) mod hierarchy;
pub(crate) mod hover;
pub(crate) mod inlay;
pub(crate) mod json_rpc;
pub(crate) mod lens;
pub(crate) mod position;
pub(crate) mod references;
pub(crate) mod state;
pub(crate) mod symbols;
pub(crate) mod tokens;
pub(crate) mod util;

const MAX_CONTENT_LENGTH: usize = 16 * 1024 * 1024; // 16MB
const MAX_DOCUMENTS: usize = 256;

/// LSP server for Mimi language
pub struct LspServer {
    pub(crate) documents: HashMap<String, String>,
    access_order: VecDeque<String>,
    workspace_root: Option<PathBuf>,
    last_cursor_line: usize,
    verification_cache: HashMap<String, (u64, VerifStatus, String)>,
    verifier: Option<Verifier>,
}

impl LspServer {
    pub fn new() -> Self {
        LspServer {
            documents: HashMap::new(),
            access_order: VecDeque::new(),
            workspace_root: None,
            last_cursor_line: 0,
            verification_cache: HashMap::new(),
            verifier: None,
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

            let len: usize = header
                .trim()
                .strip_prefix("Content-Length: ")
                .and_then(|s| s.parse().ok())
                .unwrap_or(0);

            if len == 0 || len > MAX_CONTENT_LENGTH {
                continue;
            }

            // Read JSON body
            let mut body = vec![0u8; len];
            reader
                .read_exact(&mut body)
                .map_err(|e| format!("read error: {}", e))?;
            let body = String::from_utf8(body).map_err(|e| format!("utf8 error: {}", e))?;

            // Skip empty line after body
            let mut newline = [0u8; 1];
            let _ = io::stdin().read(&mut newline);

            // Parse and handle (with panic catch to prevent server crash)
            if let Ok(msg) = serde_json::from_str::<serde_json::Value>(&body) {
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    self.handle_message(&msg)
                }));
                match result {
                    Ok(Some(response)) => {
                        let resp_str = serde_json::to_string(&response).unwrap_or_default();
                        print!("Content-Length: {}\r\n\r\n{}", resp_str.len(), resp_str);
                        io::stdout().flush().ok();
                    }
                    Ok(None) => {}
                    Err(_) => {
                        eprintln!(
                            "[lsp] handler panicked for method {:?}, continuing",
                            msg.get("method").and_then(|v| v.as_str())
                        );
                    }
                }
            }
        }
    }

    pub(crate) fn handle_message(
        &mut self,
        msg: &serde_json::Value,
    ) -> Option<serde_json::Value> {
        handlers::handle_message(self, msg)
    }
}
