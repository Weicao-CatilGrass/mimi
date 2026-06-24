use std::path::{Path, PathBuf};

/// Current working directory, captured at program startup.
///
/// Registered as a resource, injected into `#[chain]` / `#[renderer]`
/// functions that declare `&ResCurrentDir` as a parameter.
#[derive(Debug, Clone, Default)]
pub struct ResCurrentDir(pub PathBuf);

impl ResCurrentDir {
    /// Resolve source file path from CLI argument or project manifest.
    pub fn resolve_source_path(&self, arg: Option<&Path>) -> Result<PathBuf, String> {
        if let Some(path) = arg {
            return Ok(path.to_path_buf());
        }
        match mimi::manifest::Manifest::find(&self.0)? {
            Some((dir, m)) => Ok(m.entry_path(&dir)),
            None => Err("no path specified and no mimi.toml found".into()),
        }
    }
}
