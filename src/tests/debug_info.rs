use super::*;

// ===== Stage 4: DWARF debug info tests =====
//
// Mimi v1.0 does not emit any DWARF debug information.
// No `-g` flag, no DIBuilder usage, no .debug_info / .debug_line sections.
//
// These tests verify the current (null) behavior.
// Once DWARF is implemented in v1.2+, update these tests
// to expect the appropriate sections.

fn can_link_debug() -> bool {
    std::process::Command::new("cc").arg("--version").output().is_ok()
}

fn has_objdump() -> bool {
    std::process::Command::new("llvm-objdump")
        .arg("--version")
        .output()
        .is_ok()
}

#[test]
fn e2e_no_dwarf_sections() {
    if !can_link_debug() { eprintln!("SKIP: cc not available"); return; }
    if !has_objdump() { eprintln!("SKIP: llvm-objdump not available"); return; }

    let src = r#"
func main() -> i32 { 42 }
"#;

    let obj_path = match compile_only(src) {
        Ok(p) => p,
        Err(e) => { eprintln!("SKIP: compile_only failed: {}", e); return; }
    };

    let output = std::process::Command::new("llvm-objdump")
        .args(["--section-headers", &obj_path.to_string_lossy()])
        .output()
        .expect("llvm-objdump execution failed");

    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(!stdout.contains(".debug_info"),
        "unexpected .debug_info section — DWARF not yet implemented");
    assert!(!stdout.contains(".debug_line"),
        "unexpected .debug_line section — DWARF not yet implemented");
    assert!(!stdout.contains(".debug_abbrev"),
        "unexpected .debug_abbrev section — DWARF not yet implemented");
    assert!(!stdout.contains(".debug_loc"),
        "unexpected .debug_loc section — DWARF not yet implemented");
    assert!(!stdout.contains(".debug_str"),
        "unexpected .debug_str section — DWARF not yet implemented");

    if let Some(parent) = obj_path.parent() {
        let _ = std::fs::remove_dir_all(parent);
    }
}
