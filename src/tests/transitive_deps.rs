use std::fs;
use std::path::PathBuf;

use super::*;

fn setup_registry_pkg(reg: &PathBuf, name: &str, version: &str, deps: &[(&str, &str)]) {
    let pkg_dir = reg.join(name).join(version);
    fs::create_dir_all(&pkg_dir).expect("create pkg dir");

    let mut toml = format!(r#"[package]
name = "{}"
version = "{}"
entry = "main.mimi"
"#, name, version);

    if !deps.is_empty() {
        for (dep_name, dep_ver) in deps {
            toml.push_str(&format!(r#"[[dependencies]]
name = "{}"
version = "{}"

"#, dep_name, dep_ver));
        }
    }

    fs::write(pkg_dir.join("mimi.toml"), &toml).expect("write mimi.toml");
    fs::write(pkg_dir.join("main.mimi"), format!("func {}() {{}}", name)).expect("write main.mimi");
}

#[test]
fn transitive_resolution_basic() {
    let root = std::env::temp_dir().join("mimi_transitive_test_basic");
    let reg = root.join("registry");
    let project = root.join("project");

    fs::create_dir_all(&reg).expect("create reg");
    fs::create_dir_all(&project).expect("create project");

    // Setup: leaf (no deps) <- middle (depends on leaf) <- root (depends on middle)
    setup_registry_pkg(&reg, "leaf", "1.0.0", &[]);
    setup_registry_pkg(&reg, "middle", "1.0.0", &[("leaf", "^1.0")]);

    // Create root project depending on middle
    let mut manifest = crate::manifest::Manifest::new("root");
    manifest.dependencies = Some(vec![
        crate::manifest::Dependency {
            name: "middle".into(),
            version: Some("^1.0".into()),
            path: None,
            git: None,
            tag: None,
        },
    ]);
    manifest.save(&project).expect("save manifest");
    fs::write(project.join("main.mimi"), "func main() {}").expect("write main.mimi");

    // Run transitive install
    let result = super::main_install_transitive(&project, &reg);
    assert!(result.is_ok(), "transitive install should succeed: {:?}", result.err());

    // Verify lockfile contains both middle and leaf
    let lock = crate::lockfile::Lockfile::load(&project)
        .expect("load lockfile")
        .expect("lockfile should exist after install");

    let middle_entry = lock.get_package("middle");
    assert!(middle_entry.is_some(), "lockfile should contain middle");
    assert_eq!(middle_entry.unwrap().version, "1.0.0");

    let leaf_entry = lock.get_package("leaf");
    assert!(leaf_entry.is_some(), "lockfile should contain leaf (transitive dep)");
    assert_eq!(leaf_entry.unwrap().version, "1.0.0");

    // Cleanup
    fs::remove_dir_all(&root).ok();
}

#[test]
fn transitive_resolution_cycle_detection() {
    let root = std::env::temp_dir().join("mimi_transitive_test_cycle");
    let reg = root.join("registry");
    let project = root.join("project");

    fs::create_dir_all(&reg).expect("create reg");
    fs::create_dir_all(&project).expect("create project");

    // Setup: a depends on b, b depends on a (cycle)
    setup_registry_pkg(&reg, "pkg-a", "1.0.0", &[("pkg-b", "^1.0")]);
    setup_registry_pkg(&reg, "pkg-b", "1.0.0", &[("pkg-a", "^1.0")]);

    let mut manifest = crate::manifest::Manifest::new("root");
    manifest.dependencies = Some(vec![
        crate::manifest::Dependency {
            name: "pkg-a".into(),
            version: Some("^1.0".into()),
            path: None,
            git: None,
            tag: None,
        },
    ]);
    manifest.save(&project).expect("save manifest");
    fs::write(project.join("main.mimi"), "func main() {}").expect("write main.mimi");

    // Should still succeed (cycle detection should break the loop)
    let result = super::main_install_transitive(&project, &reg);
    assert!(result.is_ok(), "transitive install should handle cycles: {:?}", result.err());

    let lock = crate::lockfile::Lockfile::load(&project)
        .expect("load lockfile")
        .expect("lockfile should exist");

    assert!(lock.get_package("pkg-a").is_some(), "lockfile should contain pkg-a");
    assert!(lock.get_package("pkg-b").is_some(), "lockfile should contain pkg-b");

    // Cleanup
    fs::remove_dir_all(&root).ok();
}
