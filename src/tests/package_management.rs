use super::*;

// ===================== Lockfile Tests =====================

#[test]
fn lockfile_create_and_save() {
    let dir = std::env::temp_dir().join("mimi_lockfile_test");
    std::fs::create_dir_all(&dir).unwrap();

    let mut lf = crate::lockfile::Lockfile::new();
    lf.add_package("foo", "1.0.0", Some("git+https://example.com"), None);
    lf.add_package("bar", "2.5.0", None, Some("sha256:abc123"));

    lf.save(&dir).unwrap();

    let loaded = crate::lockfile::Lockfile::load(&dir).unwrap().unwrap();
    assert_eq!(loaded.package.len(), 2);
    assert_eq!(loaded.package[0].name, "foo");
    assert_eq!(loaded.package[0].version, "1.0.0");
    assert_eq!(loaded.package[1].name, "bar");
    assert_eq!(loaded.package[1].version, "2.5.0");

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn lockfile_resolve_version_caret() {
    let available = ["0.1.0", "0.2.0", "1.0.0", "1.1.0", "2.0.0"];
    assert_eq!(
        crate::lockfile::Lockfile::resolve_version("^1.0", &available),
        Some("1.1.0".into())
    );
}

#[test]
fn lockfile_resolve_version_exact() {
    let available = ["0.1.0", "1.0.0", "2.0.0"];
    assert_eq!(
        crate::lockfile::Lockfile::resolve_version("1.0.0", &available),
        Some("1.0.0".into())
    );
}

#[test]
fn lockfile_resolve_version_wildcard() {
    let available = ["0.1.0", "1.0.0"];
    assert_eq!(
        crate::lockfile::Lockfile::resolve_version("*", &available),
        Some("1.0.0".into())
    );
}

#[test]
fn lockfile_resolve_version_tilde() {
    let available = ["1.0.0", "1.0.1", "1.0.2", "1.1.0", "2.0.0"];
    assert_eq!(
        crate::lockfile::Lockfile::resolve_version("~1.0", &available),
        Some("1.0.2".into())
    );
}

#[test]
fn lockfile_resolve_version_range() {
    let available = ["0.1.0", "0.5.0", "1.0.0", "1.5.0", "2.0.0"];
    assert_eq!(
        crate::lockfile::Lockfile::resolve_version(">=0.5, <2.0", &available),
        Some("1.5.0".into())
    );
}

// ===================== Manifest Tests =====================

#[test]
fn manifest_add_dependency() {
    let mut manifest = crate::manifest::Manifest::new("test-pkg");
    manifest.add_dependency("foo", Some("^1.0"), None);
    manifest.add_dependency("bar", Some("2.0.0"), Some("./local"));

    let deps = manifest.dependencies.unwrap();
    assert_eq!(deps.len(), 2);
    assert_eq!(deps[0].name, "foo");
    assert_eq!(deps[0].version, Some("^1.0".into()));
    assert_eq!(deps[1].name, "bar");
    assert_eq!(deps[1].path, Some("./local".into()));
}

#[test]
fn manifest_remove_dependency() {
    let mut manifest = crate::manifest::Manifest::new("test-pkg");
    manifest.add_dependency("foo", Some("1.0"), None);
    manifest.add_dependency("bar", Some("2.0"), None);

    assert!(manifest.remove_dependency("foo"));
    assert!(!manifest.remove_dependency("foo"));

    let deps = manifest.dependencies.unwrap();
    assert_eq!(deps.len(), 1);
    assert_eq!(deps[0].name, "bar");
}

#[test]
fn manifest_replace_dependency() {
    let mut manifest = crate::manifest::Manifest::new("test-pkg");
    manifest.add_dependency("foo", Some("1.0"), None);
    manifest.add_dependency("foo", Some("2.0"), None);

    let deps = manifest.dependencies.unwrap();
    assert_eq!(deps.len(), 1);
    assert_eq!(deps[0].version, Some("2.0".into()));
}

// ===================== Multi-file Build Tests =====================

#[test]
fn codegen_multi_file_build() {
    let src = r#"
        func add(a: i32, b: i32) -> i32 {
            a + b
        }
        func main() -> i32 {
            add(1, 2)
        }
    "#;
    let file = parse(src);
    let context = inkwell::context::Context::create();
    let mut codegen = crate::codegen::CodeGenerator::new(&context, "test");
    codegen.compile_file(&file).unwrap();
    let ir = codegen.emit_ir();
    assert!(ir.contains("add"), "IR should contain add function");
    assert!(ir.contains("main"), "IR should contain main function");
}

// ===================== Test Framework Enhancement Tests =====================

#[test]
fn test_framework_assert_eq() {
    let src = r#"
        func test_addition() {
            assert_eq(1 + 1, 2)
        }
    "#;
    let file = parse(src);
    let mut interp = interp::Interpreter::new(&file);
    interp.verify_contracts = true;
    let result = interp.call_named("test_addition", vec![]);
    assert!(result.is_ok());
}

#[test]
fn test_framework_assert_ne() {
    let src = r#"
        func test_not_equal() {
            assert_ne(1, 2)
        }
    "#;
    let file = parse(src);
    let mut interp = interp::Interpreter::new(&file);
    interp.verify_contracts = true;
    let result = interp.call_named("test_not_equal", vec![]);
    assert!(result.is_ok());
}

#[test]
fn test_framework_assert_ne_fails() {
    let src = r#"
        func test_equal() {
            assert_ne(1, 1)
        }
    "#;
    let file = parse(src);
    let mut interp = interp::Interpreter::new(&file);
    interp.verify_contracts = true;
    let result = interp.call_named("test_equal", vec![]);
    assert!(result.is_err());
}
