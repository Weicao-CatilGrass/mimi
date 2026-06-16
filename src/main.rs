#![allow(dead_code)]

mod ast;
mod contracts;
mod core;
mod interp;
mod lexer;
mod loader;
mod manifest;
mod parser;
#[cfg(test)]
mod tests;

use clap::{Parser, Subcommand};
use contracts::Contract;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use crate::ast::{File, Item, Stmt};

#[derive(Parser, Debug)]
#[command(name = "mimi", version = "0.1.1", about = "Mimi language driver")]
struct Args {
    #[command(subcommand)]
    cmd: Command,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Parse and type-check a .mimi file (v0.1: parse only)
    Check {
        path: Option<PathBuf>,
        /// Extract and display contracts from mms blocks
        #[arg(long)]
        extract_contracts: bool,
        /// Strict mode: enforce $$ lock semantics
        #[arg(long)]
        strict: bool,
    },
    /// Parse and run a .mimi file
    Run {
        path: Option<PathBuf>,
        /// Enable runtime contract verification
        #[arg(long)]
        verify_contracts: bool,
    },
}

fn main() {
    let args = Args::parse();
    let result = match args.cmd {
        Command::Check { path, extract_contracts, strict } => check(path.as_deref(), extract_contracts, strict),
        Command::Run { path, verify_contracts } => run(path.as_deref(), verify_contracts),
    };
    if let Err(e) = result {
        eprintln!("error: {}", e);
        std::process::exit(1);
    }
}

/// Resolve the target path, either from argument or by finding mimi.toml
fn resolve_path(arg: Option<&Path>) -> Result<PathBuf, String> {
    if let Some(path) = arg {
        return Ok(path.to_path_buf());
    }
    // Search for mimi.toml
    let cwd = std::env::current_dir().map_err(|e| format!("cannot get cwd: {}", e))?;
    match manifest::Manifest::find(&cwd)? {
        Some((dir, m)) => Ok(m.entry_path(&dir)),
        None => Err("no path specified and no mimi.toml found".into()),
    }
}

fn is_sketch(path: &Path) -> bool {
    path.extension().map(|e| e == "mms").unwrap_or(false)
}

fn is_production(path: &Path) -> bool {
    path.extension().map(|e| e == "mimi").unwrap_or(false)
}

/// Extract contracts from all mms blocks in the file, keyed by function name
fn extract_all_contracts(file: &File) -> HashMap<String, Contract> {
    let mut result = HashMap::new();
    extract_item_contracts(&file.items, &mut result);
    result
}

fn extract_item_contracts(items: &[Item], out: &mut HashMap<String, Contract>) {
    for item in items {
        match item {
            Item::Func(func) => {
                let mut contract = Contract::default();
                for stmt in &func.body {
                    if let Stmt::MmsBlock(text) = stmt {
                        let c = contracts::extract_contracts(text);
                        contract.requires.extend(c.requires);
                        contract.ensures.extend(c.ensures);
                        contract.math.extend(c.math);
                    }
                }
                if !contract.requires.is_empty() || !contract.ensures.is_empty() || !contract.math.is_empty() {
                    out.insert(func.name.clone(), contract);
                }
            }
            Item::Module(m) => {
                extract_item_contracts(&m.items, out);
            }
            _ => {}
        }
    }
}

fn check(path: Option<&Path>, extract_contracts: bool, strict: bool) -> Result<(), String> {
    let path = resolve_path(path)?;
    let source = fs::read_to_string(&path)
        .map_err(|e| format!("failed to read {}: {}", path.display(), e))?;
    let sketch = is_sketch(&path);
    let tokens = if sketch {
        lexer::Lexer::new_sketch(&source).tokenize()?
    } else {
        lexer::Lexer::new(&source).tokenize()?
    };
    let mut file = if sketch {
        parser::Parser::new_sketch(tokens).parse_file()?
    } else {
        parser::Parser::new(tokens).parse_file()?
    };
    if sketch {
        println!("✓ {} parsed successfully (sketch mode)", path.display());
        return Ok(());
    }
    if !is_production(&path) {
        return Err(format!(
            "expected .mimi production file or .mms sketch file, got {}",
            path.display()
        ));
    }

    // Extract contracts from mms blocks if requested
    if extract_contracts {
        let contracts = extract_all_contracts(&file);
        if contracts.is_empty() {
            println!("No contracts found in mms blocks.");
        } else {
            println!("Contracts extracted from mms blocks:");
            for (func_name, contract) in &contracts {
                println!("  {}:", func_name);
                for req in &contract.requires {
                    println!("    requires: {}", req);
                }
                for ens in &contract.ensures {
                    println!("    ensures: {}", ens);
                }
                for m in &contract.math {
                    println!("    math: {}", m);
                }
            }
        }
        // Bind contracts to functions
        contracts::bind_contracts(&mut file, contracts);
    }

    let check_result = if strict {
        core::check_strict(&file)
    } else {
        core::check(&file)
    };
    if let Err(diagnostics) = check_result {
        eprintln!("✗ {} has {} type error(s):", path.display(), diagnostics.len());
        for d in diagnostics {
            eprintln!("  - {}", d.message);
        }
        return Err("type checking failed".into());
    }
    println!("✓ {} checked successfully", path.display());
    Ok(())
}

fn run(path: Option<&Path>, verify_contracts: bool) -> Result<(), String> {
    let path = resolve_path(path)?;
    let source = fs::read_to_string(&path)
        .map_err(|e| format!("failed to read {}: {}", path.display(), e))?;
    if is_sketch(&path) {
        return Err("cannot run a .mms sketch file directly; promote to .mimi first".into());
    }
    if !is_production(&path) {
        return Err(format!(
            "expected .mimi production file, got {}",
            path.display()
        ));
    }
    let tokens = lexer::Lexer::new(&source).tokenize()?;
    let file = parser::Parser::new(tokens).parse_file()?;

    // Load imports if any
    let merged_file = if !file.imports.is_empty() {
        let base_dir = path.parent().unwrap_or_else(|| std::path::Path::new(".")).to_path_buf();
        let mut loader = loader::ModuleLoader::new(base_dir);
        loader.load_main(&path)?;
        loader.merge_all()
    } else {
        file
    };

    if let Err(diagnostics) = core::check(&merged_file) {
        eprintln!("✗ {} has {} type error(s):", path.display(), diagnostics.len());
        for d in diagnostics {
            eprintln!("  - {}", d.message);
        }
        return Err("type checking failed".into());
    }
    let mut interp = interp::Interpreter::new(&merged_file);
    interp.verify_contracts = verify_contracts;
    let value = interp.run()?;
    println!("-> {}", value);
    Ok(())
}
