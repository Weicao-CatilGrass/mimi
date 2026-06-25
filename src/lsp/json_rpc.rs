#![allow(dead_code)]

use std::io::{self, BufRead, Write};

use serde_json::Value;

use crate::lsp::MAX_CONTENT_LENGTH;

/// Read a single JSON-RPC message from the given reader.
pub(crate) fn read_message<R: BufRead>(
    reader: &mut R,
    header: &mut String,
) -> io::Result<Option<Value>> {
    loop {
        header.clear();
        if reader.read_line(header)? == 0 || header.is_empty() {
            return Ok(None);
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
        return Ok(None);
    }

    let mut body = vec![0u8; len];
    reader.read_exact(&mut body)?;
    let text =
        String::from_utf8(body).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    serde_json::from_str(&text)
        .map(Some)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
}

/// Write a JSON-RPC message to stdout with the required Content-Length header.
pub(crate) fn write_message(value: &Value) -> io::Result<()> {
    let text = serde_json::to_string(value).unwrap_or_default();
    print!("Content-Length: {}\r\n\r\n{}", text.len(), text);
    io::stdout().flush()
}

/// Parse a JSON value into a JSON-RPC request representation.
pub(crate) fn parse_request(value: &Value) -> Option<(&str, Option<&Value>, Option<&Value>)> {
    let method = value.get("method")?.as_str()?;
    let id = value.get("id");
    let params = value.get("params");
    Some((method, id, params))
}

/// Send a JSON-RPC response for the given request id and result.
pub(crate) fn send_response(id: &Value, result: Value) -> io::Result<()> {
    write_message(&serde_json::json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": result
    }))
}

/// Send a JSON-RPC notification with the given method and params.
pub(crate) fn send_notification(method: &str, params: Value) -> io::Result<()> {
    write_message(&serde_json::json!({
        "jsonrpc": "2.0",
        "method": method,
        "params": params
    }))
}
