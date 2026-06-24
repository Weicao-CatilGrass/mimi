use clap::Parser;
use mingling::macros::dispatcher_clap;
use mingling::macros::route;
use mingling::prelude::*;
use mingling::Groupped;
use serde::Serialize;
use std::path::PathBuf;

use crate::{errors, resources, Next};

// ── Dispatcher ────────────────────────────────────────

#[derive(Default, Parser, Groupped, Serialize)]
#[dispatcher_clap("stats", CMDStats, help = true)]
pub struct EntryStats {
    /// Path to the .mimi or .mms file
    pub path: Option<PathBuf>,
}

// ── Result type ───────────────────────────────────────

pack!(StatsResult = StatsData);

#[derive(Debug, Clone, Default, Serialize)]
pub struct StatsData {
    pub path: String,
    pub total_items: usize,
    pub functions: usize,
    pub types: usize,
    pub modules: usize,
    pub lines: usize,
}

// ── Chain ─────────────────────────────────────────────

#[chain]
pub fn handle_stats(args: EntryStats, current_dir: &resources::ResCurrentDir) -> Next {
    let path = route!(current_dir.resolve_source_path(args.path.as_deref()).map_err(|e| {
        errors::ErrorSourceResolve { detail: e }
    }));
    let source = route!(std::fs::read_to_string(&path).map_err(|e| {
        errors::ErrorFileRead {
            path: path.display().to_string(),
            detail: e.to_string(),
        }
    }));
    let tokens = route!(mimi::lexer::Lexer::new(&source).tokenize().map_err(|e| {
        errors::ErrorParse {
            path: path.display().to_string(),
            detail: e.to_string(),
        }
    }));
    let file = route!(mimi::parser::Parser::new(tokens).parse_file().map_err(|e| {
        errors::ErrorParse {
            path: path.display().to_string(),
            detail: e.message,
        }
    }));

    let func_count = file
        .items
        .iter()
        .filter(|i| matches!(i, mimi::ast::Item::Func(_)))
        .count();
    let type_count = file
        .items
        .iter()
        .filter(|i| matches!(i, mimi::ast::Item::Type(_)))
        .count();
    let module_count = file
        .items
        .iter()
        .filter(|i| matches!(i, mimi::ast::Item::Module(_)))
        .count();
    let total = file.items.len();

    StatsResult::new(StatsData {
        path: path.display().to_string(),
        total_items: total,
        functions: func_count,
        types: type_count,
        modules: module_count,
        lines: source.lines().count(),
    })
    .to_render()
}

// ── Renderers ─────────────────────────────────────────

#[renderer]
pub fn render_stats(result: StatsResult) {
    let data = &*result;
    r_println!("Mimi source statistics for {}:", data.path);
    r_println!("  total items: {}", data.total_items);
    r_println!("  functions:   {}", data.functions);
    r_println!("  types:       {}", data.types);
    r_println!("  modules:     {}", data.modules);
    r_println!("  lines:       {}", data.lines);
}

#[renderer]
pub fn render_source_resolve_error(err: errors::ErrorSourceResolve) {
    r_println!("{}", err.detail);
}

#[renderer]
pub fn render_file_read_error(err: errors::ErrorFileRead) {
    r_println!("failed to read {}: {}", err.path, err.detail);
}

#[renderer]
pub fn render_parse_error(err: errors::ErrorParse) {
    r_println!("{}: {}", err.path, err.detail);
}

#[renderer]
pub fn render_dispatcher_not_found(err: crate::ErrorDispatcherNotFound) {
    r_println!("Command not found: [{}]", err.join(" "));
}
