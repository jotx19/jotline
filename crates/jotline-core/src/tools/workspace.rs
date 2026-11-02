use super::{fail, Failure};
use std::path::{Component, Path, PathBuf};

/// Directory names skipped by tree listing, file search, and content search.
pub(crate) const IGNORED_DIRS: [&str; 11] = [
    ".git",
    "target",
    "node_modules",
    "__pycache__",
    ".venv",
    "venv",
    ".jotline",
    ".idea",
    ".next",
    "dist",
    ".mypy_cache",
];

/// The workspace boundary. Every tool path goes through it.
#[derive(Clone, Debug)]
pub struct Workspace {
    root: PathBuf,
}

impl Workspace {
    /// Canonicalizes `root` when it exists, so later prefix checks compare real paths.
    pub fn new(root: &Path) -> Self {
        Self {
            root: root.canonicalize().unwrap_or_else(|_| root.to_path_buf()),
        }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Resolves a workspace-relative path that may not exist yet. Rejects absolute paths, `..`
    /// components, and paths whose nearest existing ancestor resolves outside the root (for
    /// example through a symlink).
    pub(crate) fn resolve(&self, path: &str) -> Result<PathBuf, Failure> {
        let rel = Path::new(path.trim());
        if rel.is_absolute()
            || rel
                .components()
                .any(|component| matches!(component, Component::ParentDir | Component::Prefix(_)))
        {
            return fail(format!(
                "path escapes workspace root: '{path}'. Use a path relative to the workspace."
            ));
        }

        let candidate = self.root.join(rel);
        if candidate.exists() {
            let canonical = candidate
                .canonicalize()
                .map_err(|err| Failure::Invalid(format!("failed to resolve '{path}': {err}")))?;
            if !canonical.starts_with(&self.root) {
                return fail(format!("path escapes workspace root: '{path}'"));
            }
            return Ok(canonical);
        }

        let mut ancestor = candidate.parent();
        while let Some(dir) = ancestor {
            if dir.exists() {
                let canonical = dir.canonicalize().map_err(|err| {
                    Failure::Invalid(format!("failed to resolve '{path}': {err}"))
                })?;
                if !canonical.starts_with(&self.root) {
                    return fail(format!("path escapes workspace root: '{path}'"));
                }
                return Ok(candidate);
            }
            ancestor = dir.parent();
        }
        fail(format!("path escapes workspace root: '{path}'"))
    }

    pub(crate) fn existing(&self, path: &str) -> Result<PathBuf, Failure> {
        let resolved = self.resolve(path)?;
        if !resolved.exists() {
            return fail(format!(
                "path does not exist: '{path}'. Use find_files or list_files to locate it."
            ));
        }
        Ok(resolved)
    }

    pub(crate) fn existing_file(&self, path: &str) -> Result<PathBuf, Failure> {
        let resolved = self.existing(path)?;
        if !resolved.is_file() {
            return fail(format!("not a file: '{path}'"));
        }
        Ok(resolved)
    }

    /// The workspace-relative form of `path` with `/` separators; `.` for the root itself.
    pub(crate) fn relative(&self, path: &Path) -> PathBuf {
        match path.strip_prefix(&self.root) {
            Ok(stripped) if stripped.as_os_str().is_empty() => PathBuf::from("."),
            Ok(stripped) => stripped.to_path_buf(),
            Err(_) => path.to_path_buf(),
        }
    }

    pub(crate) fn display(&self, path: &Path) -> String {
        self.relative(path).to_string_lossy().replace('\\', "/")
    }
}

pub(crate) fn is_ignored_dir(name: &str) -> bool {
    IGNORED_DIRS.contains(&name)
}
