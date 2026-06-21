#![allow(dead_code)]

mod ctx;
mod expr;
mod func;
mod helpers;

pub mod ffi;

#[allow(unused_imports)]
pub use ctx::{Counterexample, VerificationResult, VerifStatus, Verifier};
pub(crate) use ctx::Z3VarMap;

use crate::ast::File;

/// Verify contracts in source text.
pub fn verify_source(source: &str) -> Result<Vec<VerificationResult>, String> {
    let tokens = crate::lexer::Lexer::new(source).tokenize()?;
    let file = crate::parser::Parser::new(tokens)
        .parse_file()
        .map_err(|e| e.message)?;
    let mut verifier = match Verifier::new() {
        Ok(v) => v,
        Err(_) => return Ok(helpers::mock_verify_file(&file)),
    };
    Ok(verifier.verify_file(&file))
}

/// Verify contracts in a parsed file (supports pre-merged imports).
pub fn verify_file(file: &File) -> Result<Vec<VerificationResult>, String> {
    let mut verifier = match Verifier::new() {
        Ok(v) => v,
        Err(_) => return Ok(helpers::mock_verify_file(file)),
    };
    Ok(verifier.verify_file(file))
}

/// Parse source and verify extern call sites using Z3.
pub fn verify_ffi_source(source: &str) -> Result<Vec<VerificationResult>, String> {
    let tokens = crate::lexer::Lexer::new(source).tokenize()?;
    let file = crate::parser::Parser::new(tokens)
        .parse_file()
        .map_err(|e| e.message)?;
    let mut verifier = match Verifier::new() {
        Ok(v) => v,
        Err(_) => return Ok(helpers::mock_verify_file(&file)),
    };
    Ok(verifier.verify_ffi_call_sites(&file))
}

/// Check whether the Z3 solver is available at runtime.
pub fn is_z3_available() -> bool {
    Verifier::new().is_ok()
}

#[cfg(test)]
mod tests;
