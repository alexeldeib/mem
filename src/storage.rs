use crate::mem::Mem;
use anyhow::{anyhow, Context, Result};
use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};

/// Storage manager for .mems/ directory.
#[derive(Debug)]
pub struct Storage {
    /// Root directory (.mems/)
    root: PathBuf,
}

impl Storage {
    /// Create a new Storage pointing to the given root directory.
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }

    /// Find .mems/ in current or parent directories, or return error.
    pub fn find() -> Result<Self> {
        let mut current = std::env::current_dir()?;

        loop {
            let mems_dir = current.join(".mems");
            if mems_dir.is_dir() {
                return Ok(Self::new(mems_dir));
            }

            if !current.pop() {
                return Err(anyhow!(
                    "no .mems/ directory found (run `mem init` to create one)"
                ));
            }
        }
    }

    /// Initialize a new .mems/ directory in the current directory.
    pub fn init() -> Result<Self> {
        let current = std::env::current_dir()?;
        let mems_dir = current.join(".mems");

        if mems_dir.exists() {
            return Err(anyhow!(".mems/ already exists"));
        }

        fs::create_dir(&mems_dir).context("failed to create .mems/")?;
        fs::create_dir(mems_dir.join("archive")).context("failed to create .mems/archive/")?;

        Ok(Self::new(mems_dir))
    }

    /// Get the root path.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Convert a mem path to a file path.
    fn mem_path(&self, path: &str) -> PathBuf {
        self.root.join(format!("{path}.md"))
    }

    /// Write a file atomically (temp file + rename).
    fn write_atomic(&self, path: &Path, content: &str) -> Result<()> {
        let parent = path.parent().ok_or_else(|| anyhow!("invalid path"))?;

        // Ensure parent directories exist
        if !parent.exists() {
            fs::create_dir_all(parent).context("failed to create parent directories")?;
        }

        // Generate temp file name
        let rand: u32 = rand_u32();
        let temp_name = format!(
            "{}.{rand:08x}.tmp",
            path.file_name().and_then(|n| n.to_str()).unwrap_or("file")
        );
        let temp_path = parent.join(temp_name);

        // Write to temp file
        let mut file = File::create(&temp_path).context("failed to create temp file")?;
        file.write_all(content.as_bytes())
            .context("failed to write content")?;
        file.sync_all().context("failed to sync file")?;
        drop(file);

        // Atomic rename
        fs::rename(&temp_path, path).context("failed to rename temp file")?;

        Ok(())
    }

    /// Write a mem to disk.
    pub fn write_mem(&self, mem: &Mem) -> Result<()> {
        let path = self.mem_path(mem.path.to_str().ok_or_else(|| anyhow!("invalid path"))?);
        let content = mem.serialize()?;
        self.write_atomic(&path, &content)
    }

    /// Read a mem from disk.
    pub fn read_mem(&self, path: &str) -> Result<Mem> {
        let file_path = self.mem_path(path);

        if !file_path.exists() {
            return Err(anyhow!("mem not found: {path}"));
        }

        let content = fs::read_to_string(&file_path).context("failed to read file")?;
        Mem::parse(PathBuf::from(path), &content)
    }

    /// Check if a mem exists.
    pub fn exists(&self, path: &str) -> bool {
        self.mem_path(path).exists()
    }

    /// Delete a mem and clean up empty parent directories.
    pub fn delete_mem(&self, path: &str) -> Result<()> {
        let file_path = self.mem_path(path);

        if !file_path.exists() {
            return Err(anyhow!("mem not found: {path}"));
        }

        fs::remove_file(&file_path).context("failed to delete file")?;

        // Clean up empty parent directories (but not .mems/ itself)
        let mut parent = file_path.parent();
        while let Some(p) = parent {
            if p == self.root {
                break;
            }
            if p.read_dir()
                .map(|mut d| d.next().is_none())
                .unwrap_or(false)
            {
                fs::remove_dir(p).ok();
                parent = p.parent();
            } else {
                break;
            }
        }

        Ok(())
    }

    /// List all mems in the storage (excluding archive).
    pub fn list_mems(&self) -> Result<Vec<Mem>> {
        self.list_mems_in(&self.root, "")
    }

    /// List mems under a specific path.
    pub fn list_mems_under(&self, prefix: &str) -> Result<Vec<Mem>> {
        let dir = self.root.join(prefix);
        if !dir.exists() {
            return Ok(Vec::new());
        }
        self.list_mems_in(&dir, prefix)
    }

    fn list_mems_in(&self, dir: &Path, prefix: &str) -> Result<Vec<Mem>> {
        let mut mems = Vec::new();

        if !dir.is_dir() {
            return Ok(mems);
        }

        for entry in fs::read_dir(dir).context("failed to read directory")? {
            let entry = entry?;
            let path = entry.path();
            let name = entry.file_name();
            let name_str = name.to_string_lossy();

            // Skip archive directory at root level
            if prefix.is_empty() && name_str == "archive" {
                continue;
            }

            // Skip hidden files and temp files
            if name_str.starts_with('.') || name_str.ends_with(".tmp") {
                continue;
            }

            if path.is_dir() {
                // Recurse into subdirectory
                let sub_prefix = if prefix.is_empty() {
                    name_str.to_string()
                } else {
                    format!("{prefix}/{name_str}")
                };
                mems.extend(self.list_mems_in(&path, &sub_prefix)?);
            } else if path.extension().map(|e| e == "md").unwrap_or(false) {
                // Parse markdown file
                let mem_path = if prefix.is_empty() {
                    name_str.trim_end_matches(".md").to_string()
                } else {
                    format!("{prefix}/{}", name_str.trim_end_matches(".md"))
                };

                match self.read_mem(&mem_path) {
                    Ok(mem) => mems.push(mem),
                    Err(e) => {
                        eprintln!("warning: skipping invalid mem {mem_path}: {e}");
                    }
                }
            }
        }

        // Sort by path
        mems.sort_by(|a, b| a.path.cmp(&b.path));

        Ok(mems)
    }

    /// Move a mem to the archive.
    pub fn archive_mem(&self, path: &str) -> Result<()> {
        let src = self.mem_path(path);
        if !src.exists() {
            return Err(anyhow!("mem not found: {path}"));
        }

        let archive_path = self.root.join("archive").join(format!("{path}.md"));

        // Ensure parent directories exist in archive
        if let Some(parent) = archive_path.parent() {
            fs::create_dir_all(parent).context("failed to create archive directories")?;
        }

        fs::rename(&src, &archive_path).context("failed to move to archive")?;

        // Clean up empty parent directories
        let mut parent = src.parent();
        while let Some(p) = parent {
            if p == self.root {
                break;
            }
            if p.read_dir()
                .map(|mut d| d.next().is_none())
                .unwrap_or(false)
            {
                fs::remove_dir(p).ok();
                parent = p.parent();
            } else {
                break;
            }
        }

        Ok(())
    }
}

/// Simple random u32 using system entropy.
fn rand_u32() -> u32 {
    use std::collections::hash_map::RandomState;
    use std::hash::{BuildHasher, Hasher};

    let state = RandomState::new();
    let mut hasher = state.build_hasher();
    hasher.write_u64(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0),
    );
    hasher.finish() as u32
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn setup_storage() -> (TempDir, Storage) {
        let temp = TempDir::new().unwrap();
        let mems_dir = temp.path().join(".mems");
        fs::create_dir(&mems_dir).unwrap();
        fs::create_dir(mems_dir.join("archive")).unwrap();
        (temp, Storage::new(mems_dir))
    }

    #[test]
    fn test_write_and_read_mem() {
        let (_temp, storage) = setup_storage();

        let mem = Mem::new(
            PathBuf::from("test-doc"),
            "Test Document".to_string(),
            "Hello, world!".to_string(),
        );

        storage.write_mem(&mem).unwrap();
        let loaded = storage.read_mem("test-doc").unwrap();

        assert_eq!(loaded.title, "Test Document");
        assert_eq!(loaded.content, "Hello, world!");
    }

    #[test]
    fn test_write_creates_directories() {
        let (_temp, storage) = setup_storage();

        let mem = Mem::new(
            PathBuf::from("arch/decisions/adr-001"),
            "ADR-001".to_string(),
            "Architecture decision.".to_string(),
        );

        storage.write_mem(&mem).unwrap();
        assert!(storage.exists("arch/decisions/adr-001"));
    }

    #[test]
    fn test_delete_mem() {
        let (_temp, storage) = setup_storage();

        let mem = Mem::new(
            PathBuf::from("to-delete"),
            "Delete Me".to_string(),
            "Content".to_string(),
        );

        storage.write_mem(&mem).unwrap();
        assert!(storage.exists("to-delete"));

        storage.delete_mem("to-delete").unwrap();
        assert!(!storage.exists("to-delete"));
    }

    #[test]
    fn test_delete_cleans_empty_dirs() {
        let (_temp, storage) = setup_storage();

        let mem = Mem::new(
            PathBuf::from("a/b/c/doc"),
            "Nested".to_string(),
            "Content".to_string(),
        );

        storage.write_mem(&mem).unwrap();
        storage.delete_mem("a/b/c/doc").unwrap();

        // Parent directories should be cleaned up
        assert!(!storage.root().join("a").exists());
    }

    #[test]
    fn test_list_mems() {
        let (_temp, storage) = setup_storage();

        storage
            .write_mem(&Mem::new(
                PathBuf::from("doc1"),
                "Doc 1".to_string(),
                "Content 1".to_string(),
            ))
            .unwrap();

        storage
            .write_mem(&Mem::new(
                PathBuf::from("dir/doc2"),
                "Doc 2".to_string(),
                "Content 2".to_string(),
            ))
            .unwrap();

        let mems = storage.list_mems().unwrap();
        assert_eq!(mems.len(), 2);

        let paths: Vec<_> = mems.iter().map(|m| m.path.to_str().unwrap()).collect();
        assert!(paths.contains(&"dir/doc2"));
        assert!(paths.contains(&"doc1"));
    }

    #[test]
    fn test_list_mems_excludes_archive() {
        let (_temp, storage) = setup_storage();

        storage
            .write_mem(&Mem::new(
                PathBuf::from("active"),
                "Active".to_string(),
                "Content".to_string(),
            ))
            .unwrap();

        storage.archive_mem("active").unwrap();

        let mems = storage.list_mems().unwrap();
        assert!(mems.is_empty());
    }

    #[test]
    fn test_archive_mem() {
        let (_temp, storage) = setup_storage();

        let mem = Mem::new(
            PathBuf::from("to-archive"),
            "Archive Me".to_string(),
            "Content".to_string(),
        );

        storage.write_mem(&mem).unwrap();
        storage.archive_mem("to-archive").unwrap();

        assert!(!storage.exists("to-archive"));
        assert!(storage.root().join("archive/to-archive.md").exists());
    }

    #[test]
    fn test_archive_nested_mem() {
        let (_temp, storage) = setup_storage();

        let mem = Mem::new(
            PathBuf::from("a/b/nested"),
            "Nested".to_string(),
            "Content".to_string(),
        );

        storage.write_mem(&mem).unwrap();
        storage.archive_mem("a/b/nested").unwrap();

        assert!(!storage.exists("a/b/nested"));
        assert!(storage.root().join("archive/a/b/nested.md").exists());
    }

    #[test]
    fn test_read_nonexistent() {
        let (_temp, storage) = setup_storage();
        let result = storage.read_mem("nonexistent");
        assert!(result.is_err());
    }

    #[test]
    fn test_delete_nonexistent() {
        let (_temp, storage) = setup_storage();
        let result = storage.delete_mem("nonexistent");
        assert!(result.is_err());
    }
}
