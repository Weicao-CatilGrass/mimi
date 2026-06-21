#![allow(dead_code)]

use serde_json::Value;

use crate::span::Span;

pub(crate) fn span_to_range(span: &Span) -> Value {
    serde_json::json!({
        "start": {
            "line": span.start_line.saturating_sub(1),
            "character": span.start_col.saturating_sub(1)
        },
        "end": {
            "line": span.end_line.saturating_sub(1),
            "character": span.end_col.saturating_sub(1)
        }
    })
}

pub(crate) fn position_to_offset(text: &str, line: usize, character: usize) -> usize {
    let mut offset = 0;
    for (i, l) in text.lines().enumerate() {
        if i == line {
            return offset + character.min(l.len());
        }
        offset += l.len() + 1; // include newline
    }
    offset
}

pub(crate) fn offset_to_position(text: &str, offset: usize) -> (usize, usize) {
    let mut current = 0;
    for (i, line) in text.lines().enumerate() {
        let line_len = line.len() + 1;
        if current + line_len > offset {
            return (i, offset - current);
        }
        current += line_len;
    }
    let last_line = text.lines().count().saturating_sub(1);
    (last_line, 0)
}
