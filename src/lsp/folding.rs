use serde_json::Value;

use crate::lsp::LspServer;

impl LspServer {
    /// Compute folding ranges based on brace matching and indentation
    pub fn compute_folding_ranges(&self, text: &str) -> Vec<Value> {
        let mut ranges = Vec::new();
        let mut brace_stack: Vec<usize> = Vec::new();

        for (line_idx, line) in text.lines().enumerate() {
            for (_ch_idx, ch) in line.char_indices() {
                match ch {
                    '{' | '(' | '[' => {
                        brace_stack.push(line_idx);
                    }
                    '}' | ')' | ']' => {
                        if let Some(start_line) = brace_stack.pop() {
                            if start_line < line_idx {
                                ranges.push(serde_json::json!({
                                    "startLine": start_line,
                                    "endLine": line_idx
                                }));
                            }
                        }
                    }
                    _ => {}
                }
            }
        }

        ranges
    }
}
