use std::path::{Path, PathBuf};
use serde::Deserialize;

/// mimi.toml package configuration
#[derive(Debug, Deserialize, Clone)]
pub struct Manifest {
    pub package: Option<Package>,
    pub dependencies: Option<Vec<Dependency>>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Package {
    pub name: String,
    pub version: Option<String>,
    pub description: Option<String>,
    pub entry: Option<String>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Dependency {
    pub name: String,
    pub version: Option<String>,
    pub path: Option<String>,
}

impl Manifest {
    /// Load mimi.toml from a directory
    pub fn load(dir: &Path) -> Result<Option<Self>, String> {
        let toml_path = dir.join("mimi.toml");
        if !toml_path.exists() {
            return Ok(None);
        }
        let content = std::fs::read_to_string(&toml_path)
            .map_err(|e| format!("failed to read {}: {}", toml_path.display(), e))?;
        let manifest: Self = toml::from_str(&content)
            .map_err(|e| format!("failed to parse {}: {}", toml_path.display(), e))?;
        Ok(Some(manifest))
    }

    /// Find mimi.toml by searching up from the given path
    pub fn find(start: &Path) -> Result<Option<(PathBuf, Self)>, String> {
        let mut dir = start.to_path_buf();
        if dir.is_file() {
            dir = dir.parent().unwrap_or(&dir).to_path_buf();
        }
        loop {
            if let Some(manifest) = Self::load(&dir)? {
                return Ok(Some((dir, manifest)));
            }
            if !dir.pop() {
                return Ok(None);
            }
        }
    }

    /// Get the entry point file path
    pub fn entry_path(&self, base_dir: &Path) -> PathBuf {
        let entry = self.package.as_ref()
            .and_then(|p| p.entry.as_deref())
            .unwrap_or("main.mimi");
        base_dir.join(entry)
    }
}
